//! The window's widget tree: every page hung off the view stack, the header,
//! the drop target, and the `AppWidgets` handles that `render` writes into.

use std::path::PathBuf;

use gtk4::prelude::*;
use libadwaita::prelude::*;
use relm4::ComponentSender;

use crate::app::{App, Msg};
use crate::history;

use super::card::CardWidgets;
use super::history_page::build_history;
use super::receiving::build_receiving;
use super::resolved_icon;
use super::send::build_send;
use super::settings::build_settings;
use super::style::install_style;

pub struct AppWidgets {
    pub notice_banner: libadwaita::Banner,
    /// The notice an expiry timer has already been started for.
    pub notice_scheduled: u64,

    // Receiving
    pub incoming: CardWidgets,
    pub text_card: gtk4::Box,
    pub text_card_title: gtk4::Label,
    pub text_card_body: gtk4::Label,
    pub empty_state: libadwaita::StatusPage,
    pub empty_action: gtk4::Button,

    // History
    pub filter: gtk4::DropDown,
    pub filter_options: Vec<history::Category>,
    pub filter_handler: Option<gtk4::glib::SignalHandlerId>,
    pub history_list: gtk4::ListBox,
    pub history_revision: u64,
    pub history_empty: libadwaita::StatusPage,
    pub history_scroll: gtk4::ScrolledWindow,

    // Send
    pub outgoing: CardWidgets,
    pub discovering_row: libadwaita::SwitchRow,
    pub discovering_handler: gtk4::glib::SignalHandlerId,
    pub staged_headline: gtk4::Label,
    pub staged_detail: gtk4::Label,
    pub staged_area: gtk4::Box,
    pub pick_area: gtk4::Box,
    pub text_entry: libadwaita::EntryRow,
    pub compose_group: libadwaita::PreferencesGroup,
    pub endpoints_group: libadwaita::PreferencesGroup,
    pub endpoint_rows: std::collections::HashMap<String, libadwaita::ActionRow>,
    pub endpoint_group_leader: Option<gtk4::CheckButton>,
    pub no_devices: libadwaita::ActionRow,
    pub send_group: libadwaita::PreferencesGroup,
    pub send_button: gtk4::Button,
    pub outgoing_group: libadwaita::PreferencesGroup,
    pub retry_group: libadwaita::PreferencesGroup,

    // Settings
    pub receiving_row: libadwaita::SwitchRow,
    pub receiving_handler: gtk4::glib::SignalHandlerId,
    pub visible_as: libadwaita::ActionRow,
    pub destination_row: libadwaita::ActionRow,
    pub launch_row: libadwaita::SwitchRow,
    pub launch_handler: gtk4::glib::SignalHandlerId,
    pub sounds_row: libadwaita::SwitchRow,
    pub sounds_handler: gtk4::glib::SignalHandlerId,
    pub auto_off: libadwaita::ComboRow,
    pub auto_off_handler: gtk4::glib::SignalHandlerId,
    pub trusted_group: libadwaita::PreferencesGroup,
    pub trusted_rows: Vec<libadwaita::ActionRow>,
    pub trusted_empty: libadwaita::ActionRow,
}

/// Build every page, hang them off the window, and return the handles that
/// `render` writes into.
pub fn build(
    sender: &ComponentSender<App>,
    model: &App,
    root: &libadwaita::ApplicationWindow,
) -> AppWidgets {
    install_style(&gtk4::prelude::WidgetExt::display(root));

    let notice_banner = libadwaita::Banner::builder()
        .button_label("Dismiss")
        .build();
    {
        let sender = sender.clone();
        notice_banner.connect_button_clicked(move |_| sender.input(Msg::DismissNotice));
    }

    let (receiving_page, incoming, text_bits, empty_bits) = build_receiving(sender);
    let (history_page, history_bits) = build_history(sender);
    let (send_page, outgoing, send_bits) = build_send(sender);
    let (settings_page, settings_bits) = build_settings(sender, model);

    let stack = libadwaita::ViewStack::new();
    stack.add_titled_with_icon(
        &receiving_page,
        Some("receiving"),
        "Receiving",
        resolved_icon(&[
            "folder-download-symbolic",
            "document-save-symbolic",
            "go-down-symbolic",
        ]),
    );
    stack.add_titled_with_icon(
        &send_page,
        Some("send"),
        "Send",
        resolved_icon(&[
            "send-to-symbolic",
            "document-send-symbolic",
            "mail-send-symbolic",
            "go-up-symbolic",
        ]),
    );
    stack.add_titled_with_icon(
        &history_page,
        Some("history"),
        "History",
        resolved_icon(&["document-open-recent-symbolic", "view-list-symbolic"]),
    );
    stack.add_titled_with_icon(
        &settings_page,
        Some("settings"),
        "Settings",
        resolved_icon(&["preferences-system-symbolic", "emblem-system-symbolic"]),
    );
    stack.set_vexpand(true);

    let switcher = libadwaita::ViewSwitcher::builder()
        .policy(libadwaita::ViewSwitcherPolicy::Wide)
        .stack(&stack)
        .build();

    let header = libadwaita::HeaderBar::new();
    header.set_title_widget(Some(&switcher));

    // A primary menu, as GNOME apps have. Quit lives here only on Linux, where
    // closing the window merely hides it; elsewhere the window manager's own
    // close already quits and a second control would just duplicate it.
    {
        let menu = gtk4::gio::Menu::new();
        menu.append(Some("About DroidHarbor"), Some("app.about"));
        #[cfg(target_os = "linux")]
        menu.append(Some("Quit DroidHarbor"), Some("app.quit"));

        let menu_button = gtk4::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text("Main menu")
            .menu_model(&menu)
            .build();
        header.pack_end(&menu_button);

        // `root.application()` is still None during init — relm4 attaches the
        // window afterwards — so registering there left both items permanently
        // greyed out. The main application always exists by now.
        {
            let app = relm4::main_application();
            let about = gtk4::gio::SimpleAction::new("about", None);
            {
                let sender = sender.clone();
                about.connect_activate(move |_, _| sender.input(Msg::ShowAbout));
            }
            app.add_action(&about);

            #[cfg(target_os = "linux")]
            {
                let quit = gtk4::gio::SimpleAction::new("quit", None);
                let sender = sender.clone();
                quit.connect_activate(move |_, _| sender.input(Msg::Quit));
                app.add_action(&quit);
                app.set_accels_for_action("app.quit", &["<Primary>q"]);
            }
        }
    }

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&notice_banner);
    content.append(&stack);

    let toolbar = libadwaita::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&content));
    root.set_content(Some(&toolbar));
    // Otherwise the first entry takes focus, the page scrolls to reveal it, and
    // the top row ends up clipped under the header.
    gtk4::prelude::GtkWindowExt::set_focus(root, gtk4::Widget::NONE);

    // Dropping anywhere on the window stages a payload and switches to Send,
    // so the drop lands where the user can act on it rather than silently
    // changing state on a page they cannot see.
    {
        let drop = gtk4::DropTarget::new(gtk4::glib::Type::INVALID, gtk4::gdk::DragAction::COPY);
        drop.set_types(&[gtk4::gdk::FileList::static_type(), String::static_type()]);
        let sender = sender.clone();
        let stack = stack.clone();
        drop.connect_drop(move |_, value, _, _| {
            if let Ok(files) = value.get::<gtk4::gdk::FileList>() {
                let paths: Vec<PathBuf> = files.files().iter().filter_map(|f| f.path()).collect();
                if paths.is_empty() {
                    // A drop from a source with no local path — a remote URI,
                    // for instance — is nothing we can send.
                    return false;
                }
                sender.input(Msg::Staged(paths));
            } else if let Ok(text) = value.get::<String>() {
                if text.trim().is_empty() {
                    return false;
                }
                sender.input(Msg::StageText(text));
            } else {
                return false;
            }
            stack.set_visible_child_name("send");
            true
        });
        root.add_controller(drop);
    }

    AppWidgets {
        notice_banner,
        notice_scheduled: 0,
        incoming,
        text_card: text_bits.0,
        text_card_title: text_bits.1,
        text_card_body: text_bits.2,
        empty_state: empty_bits.0,
        empty_action: empty_bits.1,
        filter: history_bits.filter,
        filter_options: Vec::new(),
        filter_handler: None,
        history_list: history_bits.list,
        history_revision: 0,
        history_empty: history_bits.empty,
        history_scroll: history_bits.scroll,
        outgoing,
        discovering_row: send_bits.discovering_row,
        discovering_handler: send_bits.discovering_handler,
        staged_headline: send_bits.staged_headline,
        staged_detail: send_bits.staged_detail,
        staged_area: send_bits.staged_area,
        pick_area: send_bits.pick_area,
        text_entry: send_bits.text_entry,
        compose_group: send_bits.compose_group,
        endpoints_group: send_bits.endpoints_group,
        endpoint_rows: std::collections::HashMap::new(),
        endpoint_group_leader: None,
        no_devices: send_bits.no_devices,
        send_group: send_bits.send_group,
        send_button: send_bits.send_button,
        outgoing_group: send_bits.outgoing_group,
        retry_group: send_bits.retry_group,
        receiving_row: settings_bits.receiving_row,
        receiving_handler: settings_bits.receiving_handler,
        visible_as: settings_bits.visible_as,
        destination_row: settings_bits.destination_row,
        launch_row: settings_bits.launch_row,
        launch_handler: settings_bits.launch_handler,
        sounds_row: settings_bits.sounds_row,
        sounds_handler: settings_bits.sounds_handler,
        auto_off: settings_bits.auto_off,
        auto_off_handler: settings_bits.auto_off_handler,
        trusted_group: settings_bits.trusted_group,
        trusted_rows: Vec::new(),
        trusted_empty: settings_bits.trusted_empty,
    }
}
