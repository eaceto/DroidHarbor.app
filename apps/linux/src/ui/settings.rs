//! Everything configurable, plus the honest notes about visibility and the
//! unofficial protocol that belong in front of the user rather than a README.

use gtk4::prelude::*;
use libadwaita::prelude::*;
use relm4::ComponentSender;

use crate::app::{App, Msg, AUTO_OFF_CHOICES};

use super::{add_group, GROUP_GAP};

pub(super) struct SettingsBits {
    pub receiving_row: libadwaita::SwitchRow,
    pub visible_as: libadwaita::ActionRow,
    pub destination_row: libadwaita::ActionRow,
    pub launch_row: libadwaita::SwitchRow,
    pub sounds_row: libadwaita::SwitchRow,
    pub auto_off: libadwaita::ComboRow,
    pub trusted_group: libadwaita::PreferencesGroup,
    pub trusted_empty: libadwaita::ActionRow,
}

pub(super) fn build_settings(
    sender: &ComponentSender<App>,
    model: &App,
) -> (gtk4::Widget, SettingsBits) {
    let page = libadwaita::PreferencesPage::new();
    page.set_margin_bottom(GROUP_GAP);

    let receiving_group = libadwaita::PreferencesGroup::builder()
        .title("Receiving")
        .build();
    let receiving_row = libadwaita::SwitchRow::builder()
        .title("Receive files")
        .build();
    {
        let sender = sender.clone();
        receiving_row.connect_active_notify(move |row| {
            sender.input(Msg::SetReceiving(row.is_active()));
        });
    }
    let visible_as = libadwaita::ActionRow::builder().title("Visible as").build();
    let destination_row = libadwaita::ActionRow::builder().title("Save to").build();
    let change = gtk4::Button::with_label("Change…");
    change.set_valign(gtk4::Align::Center);
    {
        let sender = sender.clone();
        change.connect_clicked(move |_| sender.input(Msg::ChooseDestination));
    }
    destination_row.add_suffix(&change);
    destination_row.set_activatable_widget(Some(&change));
    receiving_group.add(&receiving_row);
    receiving_group.add(&visible_as);
    receiving_group.add(&destination_row);
    add_group(&page, &receiving_group);

    let device_group = libadwaita::PreferencesGroup::builder()
        .title("This computer")
        .description(
            "Nearby Android devices see this name in their share sheet. \
             Changing it restarts the receiver.",
        )
        .build();
    let name_entry = libadwaita::EntryRow::builder().title("Device name").build();
    name_entry.set_text(&model.device_name);
    name_entry.set_show_apply_button(true);
    {
        let sender = sender.clone();
        // Committed on the apply button or Enter rather than per keystroke,
        // since every commit restarts the receiver.
        name_entry.connect_apply(move |entry| {
            sender.input(Msg::RenameDevice(entry.text().to_string()));
        });
    }
    device_group.add(&name_entry);
    add_group(&page, &device_group);

    let general = libadwaita::PreferencesGroup::builder()
        .title("General")
        .build();
    let launch_row = libadwaita::SwitchRow::builder()
        .title("Open at login")
        .subtitle("Starts DroidHarbor when you log in")
        .build();
    {
        let sender = sender.clone();
        launch_row.connect_active_notify(move |row| {
            sender.input(Msg::SetLaunchAtLogin(row.is_active()));
        });
    }
    let auto_off = libadwaita::ComboRow::builder()
        .title("Turn receiving off when idle")
        .model(&gtk4::StringList::new(&[
            "Never",
            "After 10 minutes",
            "After 30 minutes",
            "After 1 hour",
        ]))
        .build();
    {
        let sender = sender.clone();
        auto_off.connect_selected_notify(move |row| {
            let minutes = AUTO_OFF_CHOICES
                .get(row.selected() as usize)
                .copied()
                .unwrap_or(0);
            sender.input(Msg::SetAutoOff(minutes));
        });
    }
    let sounds_row = libadwaita::SwitchRow::builder()
        .title("Play sounds")
        .subtitle("Ask the notification server for a sound when a transfer finishes")
        .build();
    {
        let sender = sender.clone();
        sounds_row.connect_active_notify(move |row| {
            sender.input(Msg::SetPlaySounds(row.is_active()));
        });
    }

    let updates_row = libadwaita::ActionRow::builder()
        .title("Check for updates")
        .subtitle("Looks for a newer release; installing stays up to you")
        .build();
    let check = gtk4::Button::with_label("Check");
    check.set_valign(gtk4::Align::Center);
    {
        let sender = sender.clone();
        check.connect_clicked(move |_| sender.input(Msg::CheckForUpdates));
    }
    updates_row.add_suffix(&check);
    updates_row.set_activatable_widget(Some(&check));

    let intro_row = libadwaita::ActionRow::builder()
        .title("Show the introduction again")
        .build();
    let intro = gtk4::Button::with_label("Show");
    intro.set_valign(gtk4::Align::Center);
    {
        let sender = sender.clone();
        intro.connect_clicked(move |_| sender.input(Msg::ShowOnboarding));
    }
    intro_row.add_suffix(&intro);
    intro_row.set_activatable_widget(Some(&intro));

    general.add(&launch_row);
    general.add(&sounds_row);
    general.add(&auto_off);
    general.add(&updates_row);
    general.add(&intro_row);

    // An AppImage is run, not installed, so nothing has registered it with the
    // desktop. Offering it here beats expecting people to write a .desktop file.
    #[cfg(target_os = "linux")]
    {
        let install_row = libadwaita::ActionRow::builder()
            .title("Add to the applications menu")
            .subtitle("Creates a launcher entry pointing at this copy of DroidHarbor")
            .build();
        let install = gtk4::Button::with_label("Add");
        install.set_valign(gtk4::Align::Center);
        {
            let sender = sender.clone();
            install.connect_clicked(move |_| sender.input(Msg::InstallDesktopEntry));
        }
        install_row.add_suffix(&install);
        install_row.set_activatable_widget(Some(&install));
        general.add(&install_row);
    }
    add_group(&page, &general);

    let trusted_group = libadwaita::PreferencesGroup::builder()
        .title("Trusted devices")
        .description(
            "Trusted transfers are accepted without the confirmation code. \
             Devices are matched by the name they announce, which a device \
             chooses for itself.",
        )
        .build();
    let trusted_empty = libadwaita::ActionRow::builder()
        .title("None yet")
        .subtitle("Tick “Always accept from this device” when accepting a transfer")
        .sensitive(false)
        .build();
    trusted_group.add(&trusted_empty);
    add_group(&page, &trusted_group);

    let privacy = libadwaita::PreferencesGroup::builder()
        .title("Privacy")
        .description(
            "While receiving is on, this computer is visible to any nearby Android device \
             with Quick Share open. Every transfer still needs your explicit acceptance, \
             and the code shown must match the phone.",
        )
        .build();
    add_group(&page, &privacy);

    let about = libadwaita::PreferencesGroup::builder()
        .description(
            "DroidHarbor implements an unofficial, reverse-engineered protocol in order to \
             interoperate with the sharing built into Android, and may stop working after an \
             Android update. Files never leave your local network.\n\n\
             Android, Google Play and Quick Share are trademarks of Google LLC. Free software \
             under the GNU General Public License v3 or later, built on rquickshare.",
        )
        .build();
    add_group(&page, &about);

    (
        page.upcast(),
        SettingsBits {
            receiving_row,
            visible_as,
            destination_row,
            launch_row,
            sounds_row,
            auto_off,
            trusted_group,
            trusted_empty,
        },
    )
}
