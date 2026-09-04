//! Send: the staged payload above the device list — the payload first, since
//! it is what the user came to send.

use gtk4::prelude::*;
use libadwaita::prelude::*;
use relm4::ComponentSender;

use crate::app::{App, Msg};

use super::card::{build_card, CardWidgets};
use super::{add_group, padded, resolved_icon, GROUP_GAP};

pub(super) struct SendBits {
    pub discovering_row: libadwaita::SwitchRow,
    pub discovering_handler: gtk4::glib::SignalHandlerId,
    pub staged_headline: gtk4::Label,
    pub staged_detail: gtk4::Label,
    pub staged_area: gtk4::Box,
    pub pick_area: gtk4::Box,
    pub text_entry: libadwaita::EntryRow,
    pub compose_group: libadwaita::PreferencesGroup,
    pub endpoints_group: libadwaita::PreferencesGroup,
    pub no_devices: libadwaita::ActionRow,
    pub send_group: libadwaita::PreferencesGroup,
    pub send_button: gtk4::Button,
    pub outgoing_group: libadwaita::PreferencesGroup,
    pub retry_group: libadwaita::PreferencesGroup,
}

pub(super) fn build_send(sender: &ComponentSender<App>) -> (gtk4::Widget, CardWidgets, SendBits) {
    let page = libadwaita::PreferencesPage::new();
    page.set_margin_bottom(GROUP_GAP);

    // The outbound transfer lives here rather than on Receiving: this is the
    // page the user was on when they started it, and where they will look to
    // call it off.
    let outgoing = build_card(sender);
    let outgoing_group = libadwaita::PreferencesGroup::new();
    outgoing_group.add(&outgoing.root);
    // Hidden as a whole, not merely emptied: a visible group still occupies its
    // slot and its margins, which pushed the first real group flush to the top.
    outgoing_group.set_visible(false);
    add_group(&page, &outgoing_group);

    let discovering_row = libadwaita::SwitchRow::builder()
        .title("Look for nearby devices")
        .subtitle("The phone must have its Quick Share screen open to be found")
        .build();
    // The handler id is kept so `render` can set the switch from the model
    // without that read back as a user action: see `sync_switch`.
    let discovering_handler = {
        let sender = sender.clone();
        discovering_row.connect_active_notify(move |row| {
            sender.input(Msg::SetDiscovering(row.is_active()));
        })
    };
    let discovery_group = libadwaita::PreferencesGroup::new();
    discovery_group.add(&discovering_row);
    add_group(&page, &discovery_group);

    let retry_row = libadwaita::ActionRow::builder()
        .title("The last send did not finish")
        .subtitle("The files are still selected; try the same device again")
        .build();
    let retry = gtk4::Button::with_label("Try again");
    retry.set_valign(gtk4::Align::Center);
    retry.add_css_class("suggested-action");
    let dismiss_retry = gtk4::Button::from_icon_name("window-close-symbolic");
    dismiss_retry.set_valign(gtk4::Align::Center);
    dismiss_retry.add_css_class("flat");
    {
        let sender = sender.clone();
        retry.connect_clicked(move |_| sender.input(Msg::RetrySend));
    }
    {
        let sender = sender.clone();
        dismiss_retry.connect_clicked(move |_| sender.input(Msg::DismissRetry));
    }
    retry_row.add_suffix(&retry);
    retry_row.add_suffix(&dismiss_retry);
    let retry_group = libadwaita::PreferencesGroup::new();
    retry_group.add(&retry_row);
    retry_group.set_visible(false);
    add_group(&page, &retry_group);

    let compose_group = libadwaita::PreferencesGroup::builder()
        .title("What to send")
        .build();

    let pick_area = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    padded(&pick_area, 18);
    let pick_icon = gtk4::Image::from_icon_name(resolved_icon(&[
        "document-send-symbolic",
        "send-to-symbolic",
        "mail-send-symbolic",
        "go-up-symbolic",
    ]));
    pick_icon.set_pixel_size(32);
    pick_icon.add_css_class("dim-label");
    let pick_hint = gtk4::Label::builder()
        .label("Choose files, or type a link or some text")
        .wrap(true)
        .build();
    pick_hint.add_css_class("dim-label");
    let choose = gtk4::Button::with_label("Choose files…");
    choose.set_halign(gtk4::Align::Center);
    {
        let sender = sender.clone();
        choose.connect_clicked(move |_| sender.input(Msg::StageFiles));
    }
    pick_area.append(&pick_icon);
    pick_area.append(&pick_hint);
    pick_area.append(&choose);
    compose_group.add(&pick_area);

    let text_entry = libadwaita::EntryRow::builder()
        .title("Text, a link, or an address")
        .build();
    {
        let sender = sender.clone();
        text_entry.connect_changed(move |entry| {
            sender.input(Msg::StageText(entry.text().to_string()));
        });
    }
    compose_group.add(&text_entry);

    let staged_area = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    padded(&staged_area, 12);
    staged_area.set_visible(false);
    let staged_text = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    staged_text.set_hexpand(true);
    let staged_headline = gtk4::Label::builder().xalign(0.0).build();
    staged_headline.add_css_class("heading");
    let staged_detail = gtk4::Label::builder().xalign(0.0).wrap(true).build();
    staged_detail.add_css_class("dim-label");
    staged_text.append(&staged_headline);
    staged_text.append(&staged_detail);
    let clear_staged = gtk4::Button::with_label("Clear");
    clear_staged.set_valign(gtk4::Align::Center);
    {
        let sender = sender.clone();
        clear_staged.connect_clicked(move |_| sender.input(Msg::ClearStaged));
    }
    staged_area.append(&staged_text);
    staged_area.append(&clear_staged);
    compose_group.add(&staged_area);
    add_group(&page, &compose_group);

    let endpoints_group = libadwaita::PreferencesGroup::builder()
        .title("Nearby devices")
        .build();
    let no_devices = libadwaita::ActionRow::builder()
        .title("No devices yet")
        .subtitle("Turn discovery on and open Quick Share on the phone")
        .sensitive(false)
        .build();
    endpoints_group.add(&no_devices);
    add_group(&page, &endpoints_group);

    let send_button = gtk4::Button::with_label("Send");
    send_button.add_css_class("suggested-action");
    send_button.add_css_class("pill");
    send_button.set_halign(gtk4::Align::Center);
    send_button.set_sensitive(false);
    {
        let sender = sender.clone();
        send_button.connect_clicked(move |_| sender.input(Msg::Send));
    }
    let send_group = libadwaita::PreferencesGroup::new();
    send_group.add(&send_button);
    add_group(&page, &send_group);

    (
        page.upcast(),
        outgoing,
        SendBits {
            discovering_row,
            discovering_handler,
            staged_headline,
            staged_detail,
            staged_area,
            pick_area,
            text_entry,
            compose_group,
            endpoints_group,
            no_devices,
            send_group,
            send_button,
            outgoing_group,
            retry_group,
        },
    )
}
