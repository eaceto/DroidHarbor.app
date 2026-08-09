//! Headless Quick Share receiver, the M0 protocol spike.
//!
//! Drives `dh-domain` through the same command/event API the desktop UIs
//! will use; the only UI here is stdout and a y/N prompt. Exit criterion for
//! M0: a file sent from an unmodified Android phone lands in the destination
//! folder with the confirmation code shown on both screens.

use std::error::Error;
use std::io::Write as _;
use std::path::PathBuf;

use clap::Parser;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::broadcast;

use dh_core::limits::Limits;
use dh_domain::{Command, DomainConfig, Event, SessionId, Settings};
use dh_qs_core::QuickShareConfig;

#[derive(Parser, Debug)]
#[command(
    name = "dh",
    about = "Receive files from Android's built-in sharing (Quick Share) on your desktop",
    long_about = None
)]
struct Args {
    /// Destination folder for received files.
    #[arg(short, long, default_value = "./received")]
    destination: PathBuf,

    /// Staging directory for in-flight files (same filesystem as the
    /// destination keeps finalization atomic).
    #[arg(long)]
    staging: Option<PathBuf>,

    /// Fixed TCP port; omit for an ephemeral one.
    #[arg(short, long)]
    port: Option<u16>,

    /// Name shown in the phone's share sheet (default: system hostname).
    #[arg(short, long)]
    name: Option<String>,

    /// Accept incoming transfers without prompting.
    #[arg(short = 'y', long)]
    yes: bool,

    /// Send these files to a nearby Android device instead of receiving.
    /// The phone must have its Quick Share screen open to be discoverable.
    #[arg(long, value_name = "FILE", num_args = 1..)]
    send: Vec<PathBuf>,

    /// Send text instead of files. A web address arrives as a link the
    /// phone can open; anything else arrives as plain text.
    #[arg(long, value_name = "TEXT", conflicts_with = "send")]
    send_text: Option<String>,
}

/// What an outbound run is carrying.
enum Outgoing {
    Files(Vec<String>),
    Text(String),
}

impl Outgoing {
    fn describe(&self) -> String {
        match self {
            Outgoing::Files(paths) => format!("{} file(s)", paths.len()),
            Outgoing::Text(_) => "some text".to_string(),
        }
    }

    fn command(&self, endpoint: String) -> Command {
        match self {
            Outgoing::Files(paths) => Command::SendFiles {
                endpoint,
                files: paths.clone(),
            },
            Outgoing::Text(text) => {
                // A web address is worth announcing as a link: the phone
                // then offers to open it rather than only to copy it.
                let is_link = text.starts_with("http://") || text.starts_with("https://");
                Command::SendText {
                    endpoint,
                    kind: if is_link {
                        "link".into()
                    } else {
                        "text".into()
                    },
                    description: text.chars().take(60).collect(),
                    content: text.clone(),
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // rqs_lib logs through `log`/`tracing`; keep its noisy dependencies down
    // unless the user overrides via RUST_LOG.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "warn,dh_qs_core=info,dh_cli=info".into());
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();
    let destination = std::path::absolute(&args.destination)?;
    let staging = match &args.staging {
        Some(dir) => std::path::absolute(dir)?,
        None => destination.join(".dh-staging"),
    };
    std::fs::create_dir_all(&destination)?;

    let limits = Limits::default();
    let (channels, frontdoor_task) = dh_qs_core::spawn(QuickShareConfig {
        staging_dir: staging.clone(),
        port: args.port,
        device_name: args.name.clone(),
        consent_timeout: std::time::Duration::from_secs(limits.accept_timeout_secs),
    })
    .await?;

    let device_name = args.name.clone().unwrap_or_else(|| "DroidHarbor".into());
    let config = DomainConfig {
        settings: Settings::new(device_name, destination.clone(), staging),
        limits,
    };
    let (handle, engine_task) = dh_domain::spawn(config, channels);
    let mut events = handle.subscribe();

    let outgoing = if let Some(text) = args.send_text.clone() {
        Some(Outgoing::Text(text))
    } else if args.send.is_empty() {
        None
    } else {
        let mut paths = Vec::with_capacity(args.send.len());
        for file in &args.send {
            let path = std::path::absolute(file)?;
            if !path.is_file() {
                return Err(format!("not a file: {}", path.display()).into());
            }
            paths.push(path.to_string_lossy().into_owned());
        }
        Some(Outgoing::Files(paths))
    };

    if let Some(outgoing) = outgoing {
        let result = send_flow(&handle, &mut events, &outgoing).await;
        handle.send(Command::Shutdown).await.ok();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), engine_task).await;
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), frontdoor_task).await;
        return result;
    }

    handle.send(Command::SetReceiving(true)).await?;

    println!("Receiving is ON. This machine is now visible to nearby Android devices");
    match &args.name {
        Some(name) => println!("(as \"{name}\", while this process runs)."),
        None => println!("(as this machine's hostname, while this process runs)."),
    }
    println!();
    println!("On the phone: select files → Share → Quick Share → tap this device.");
    println!("Saving into: {}", destination.display());
    println!("Press Ctrl+C to stop.");
    println!();

    let mut stdin = BufReader::new(tokio::io::stdin()).lines();
    // Session currently waiting for a y/N answer.
    let mut pending: Option<SessionId> = None;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("\nStopping…");
                break;
            }
            event = events.recv() => {
                match event {
                    Ok(event) => {
                        if handle_event(&handle, event, args.yes, &mut pending).await? {
                            // keep looping; receiver stays up for more sessions
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("event stream lagged, {n} events dropped");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        eprintln!("domain engine stopped unexpectedly");
                        break;
                    }
                }
            }
            line = stdin.next_line(), if pending.is_some() && !args.yes => {
                let answer = line?.unwrap_or_default();
                let session = pending.take().expect("guarded by pending.is_some()");
                if matches!(answer.trim(), "y" | "Y" | "yes") {
                    handle.send(Command::Accept(session)).await?;
                    println!("Accepted, receiving…");
                } else {
                    handle.send(Command::Decline(session)).await?;
                    println!("Declined.");
                }
            }
        }
    }

    handle.send(Command::Shutdown).await.ok();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), engine_task).await;
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), frontdoor_task).await;
    Ok(())
}

/// Interactive outbound flow: discover endpoints, let the user pick one by
/// number, send, report progress, exit when the session ends.
async fn send_flow(
    handle: &dh_domain::DomainHandle,
    events: &mut broadcast::Receiver<Event>,
    outgoing: &Outgoing,
) -> Result<(), Box<dyn Error>> {
    handle.send(Command::SetDiscovering(true)).await?;
    println!("Looking for nearby Android devices…");
    println!("On the phone: open Quick Share (Settings → Connected devices, or the Files app) so it becomes visible.");
    println!();
    println!(
        "Type a device number to send {}; Ctrl+C to abort.",
        outgoing.describe()
    );

    let mut stdin = BufReader::new(tokio::io::stdin()).lines();
    // Discovered endpoints in display order: (endpoint id, name).
    let mut endpoints: Vec<(String, String)> = Vec::new();
    let mut sending = false;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("\nAborted.");
                return Ok(());
            }
            event = events.recv() => {
                match event {
                    Ok(Event::EndpointUpdated { endpoint, name, present, .. }) => {
                        if present && !endpoints.iter().any(|(id, _)| *id == endpoint) {
                            endpoints.push((endpoint, name.clone()));
                            println!("  [{}] {name}", endpoints.len());
                        } else if !present {
                            if let Some(pos) = endpoints.iter().position(|(id, _)| *id == endpoint) {
                                println!("  [{}] {} is gone", pos + 1, endpoints[pos].1);
                            }
                        }
                    }
                    Ok(Event::SendAwaitingConsent { total_bytes, .. }) => {
                        println!("Waiting for the phone to accept ({})…", human_bytes(total_bytes));
                    }
                    Ok(Event::Progress { bytes_received, total_bytes, .. }) => {
                        if total_bytes > 0 {
                            print!(
                                "\r  {} / {} ({}%)   ",
                                human_bytes(bytes_received),
                                human_bytes(total_bytes),
                                bytes_received * 100 / total_bytes
                            );
                        } else {
                            print!("\r  {}   ", human_bytes(bytes_received));
                        }
                        let _ = std::io::stdout().flush();
                    }
                    Ok(Event::SessionEnded { outcome, .. }) => {
                        println!("\nSend ended: {outcome:?}");
                        return Ok(());
                    }
                    Ok(Event::ErrorOccurred { code, message, .. }) => {
                        eprintln!("error [{code:?}]: {message}");
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("event stream lagged, {n} events dropped");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err("domain engine stopped unexpectedly".into());
                    }
                }
            }
            line = stdin.next_line(), if !sending => {
                let input = line?.unwrap_or_default();
                let Ok(index) = input.trim().parse::<usize>() else {
                    println!("Type the number of a listed device.");
                    continue;
                };
                let Some((endpoint, name)) = endpoints.get(index.saturating_sub(1)) else {
                    println!("No device [{index}] yet.");
                    continue;
                };
                println!("Sending to \"{name}\"…");
                handle.send(outgoing.command(endpoint.clone())).await?;
                sending = true;
            }
        }
    }
}

/// React to one domain event. Returns Ok(true) to continue the loop.
async fn handle_event(
    handle: &dh_domain::DomainHandle,
    event: Event,
    auto_accept: bool,
    pending: &mut Option<SessionId>,
) -> Result<bool, Box<dyn Error>> {
    match event {
        Event::AdvertisingChanged(_) | Event::SessionConnected { .. } => {}
        Event::IntroductionReceived {
            session,
            sender_name,
            files,
            total_bytes,
            token,
            ..
        } => {
            println!("Incoming from \"{sender_name}\":");
            for file in &files {
                println!("  • {}", file.name);
            }
            println!("  {} file(s), {}", files.len(), human_bytes(total_bytes));
            println!();
            println!("  Confirmation code: {token}  ← must match the phone");
            if auto_accept {
                println!("  Auto-accepting (--yes).");
                handle.send(Command::Accept(session)).await?;
            } else {
                print!("  Accept? [y/N] ");
                let _ = std::io::stdout().flush();
                *pending = Some(session);
            }
        }
        Event::Progress {
            bytes_received,
            total_bytes,
            ..
        } => {
            if total_bytes > 0 {
                print!(
                    "\r  {} / {} ({}%)   ",
                    human_bytes(bytes_received),
                    human_bytes(total_bytes),
                    bytes_received * 100 / total_bytes
                );
            } else {
                print!("\r  {}   ", human_bytes(bytes_received));
            }
            let _ = std::io::stdout().flush();
        }
        Event::FileFinalized { path, .. } => {
            println!("\nSaved: {path}");
        }
        Event::SessionEnded { outcome, .. } => {
            println!("\nSession ended: {outcome:?}");
            println!("Waiting for the next transfer (Ctrl+C to stop)…");
            *pending = None;
        }
        Event::ErrorOccurred { code, message, .. } => {
            eprintln!("\nerror [{code:?}]: {message}");
        }
        Event::TextReceived { kind, content, .. } => {
            println!("\nReceived {kind}: {content}");
        }
        // Outbound events do not occur in receive mode.
        Event::DiscoveringChanged(_)
        | Event::EndpointUpdated { .. }
        | Event::SendAwaitingConsent { .. } => {}
    }
    Ok(true)
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::human_bytes;

    #[test]
    fn formats_byte_sizes() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.0 KiB");
        assert_eq!(human_bytes(5 * 1024 * 1024), "5.0 MiB");
        assert_eq!(human_bytes(3 * 1024 * 1024 * 1024), "3.0 GiB");
    }
}
