# DroidHarbor CLI (M0)

Headless Quick Share receiver. Drives `dh-domain` through the same
command/event API the desktop UIs will use; stdout and a y/N prompt are the
whole interface.

```sh
cargo run -p dh-cli -- --destination ~/Downloads/aft
# or accept without prompting:
cargo run -p dh-cli -- -y
# send files instead of receiving (pick a device by number once discovered):
cargo run -p dh-cli -- --send photo.jpg video.mp4

# send text or a link the same way:
cargo run -p dh-cli -- --send-text "https://example.com"
```

Flags: `--destination <dir>` (default `./received`), `--staging <dir>`
(default `<destination>/.dh-staging`), `--port <n>` (default ephemeral),
`--yes` to auto-accept. `RUST_LOG` controls verbosity.

While receiving, the machine is visible (as its hostname or `--name`) to
nearby Android devices; each transfer shows the 4-digit confirmation code
that must match the phone. Ctrl+C stops and withdraws the advertisement.

In `--send` and `--send-text` mode the phone must have its Quick Share screen
open (Settings →
Connected devices, or the Files app) to be discoverable; the phone's user
accepts the transfer there.
