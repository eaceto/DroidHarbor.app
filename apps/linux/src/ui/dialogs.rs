//! The dialogs: About, the first-run introduction, and the clear-history
//! confirmation.

use libadwaita::prelude::*;
use relm4::ComponentSender;

use crate::app::{App, Msg};
use crate::APP_ID;

/// The About panel, carrying the same credits, licence and disclaimer the
/// macOS app shows — the honest notes belong in front of the user, not only in
/// a README.
pub fn present_about(parent: &libadwaita::ApplicationWindow) {
    let about = libadwaita::AboutDialog::builder()
        .application_name("DroidHarbor")
        .application_icon(APP_ID)
        .developer_name("Ezequiel (Kimi) Aceto")
        .version(env!("CARGO_PKG_VERSION"))
        .website("https://kimi.blog")
        .issue_url("https://github.com/eaceto/DroidHarbor.app/issues")
        .license_type(gtk4::License::Gpl30)
        .comments(
            "Receive files from Android's built-in sharing, and send files back, \
             over your local network, with nothing in the cloud.\n\n\
             Implements an unofficial, reverse-engineered protocol in order to \
             interoperate with the sharing built into Android; not affiliated with \
             or endorsed by Google or Android, and it may stop working after an \
             Android update.\n\n\
             Android, Google Play and Quick Share are trademarks of Google LLC.",
        )
        .copyright("© 2026 Ezequiel Leonardo Aceto")
        .build();
    about.add_link("Source code", "https://github.com/eaceto/DroidHarbor.app");
    about.add_link("Licence", "https://www.gnu.org/licenses/gpl-3.0.html");
    about.add_credit_section(
        Some("Built on"),
        &["rquickshare https://github.com/Martichou/rquickshare"],
    );
    about.present(Some(parent));
}

/// First-run introduction, shown once and reachable again from Settings.
///
/// Kept to what someone needs before their first transfer: that the phone needs
/// no app, where files land, and that nothing leaves the local network.
pub fn present_onboarding(parent: &libadwaita::ApplicationWindow, sender: ComponentSender<App>) {
    let dialog = libadwaita::AlertDialog::new(Some("Receive files from Android"), None);
    dialog.set_body_use_markup(true);
    dialog.set_body(
        "There is <b>nothing to install on the phone</b> — DroidHarbor speaks the sharing \
         Android already has.\n\n\
         <b>To receive:</b> turn Receiving on, then on the phone pick files and choose \
         Share → Quick Share → this computer. Accept the transfer here and check the \
         four-digit code matches.\n\n\
         <b>To send:</b> open Quick Share on the phone so it can be discovered, then pick \
         files here and choose the device.\n\n\
         Transfers go directly between the two devices. Nothing is uploaded anywhere, and \
         nothing is announced until you switch receiving on.",
    );
    dialog.add_response("close", "Not now");
    dialog.add_response("start", "Turn on receiving");
    dialog.set_response_appearance("start", libadwaita::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("start"));
    dialog.set_close_response("close");

    dialog.connect_response(None, move |_, response| {
        sender.input(Msg::FinishOnboarding);
        if response == "start" {
            sender.input(Msg::SetReceiving(true));
        }
    });
    dialog.present(Some(parent));
}

/// Ask before wiping the record: unlike a removed row, there is no putting
/// the whole history back. With a filter or search active the dialog offers
/// both readings of "clear" — just what is shown, or everything.
pub fn confirm_clear_history(
    parent: &libadwaita::ApplicationWindow,
    shown: usize,
    filtered: bool,
    sender: ComponentSender<App>,
) {
    let dialog = libadwaita::AlertDialog::new(
        Some("Clear history?"),
        Some("Files stay where they were saved. Only the record of past transfers is cleared."),
    );
    dialog.add_response("cancel", "Cancel");
    if filtered {
        dialog.add_response("shown", &format!("Clear Shown ({shown})"));
        dialog.set_response_appearance("shown", libadwaita::ResponseAppearance::Destructive);
        dialog.add_response("all", "Clear All");
    } else {
        dialog.add_response("all", "Clear History");
    }
    dialog.set_response_appearance("all", libadwaita::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    dialog.connect_response(None, move |_, response| match response {
        "shown" => sender.input(Msg::ClearHistoryShown),
        "all" => sender.input(Msg::ClearHistoryAll),
        _ => {}
    });
    dialog.present(Some(parent));
}
