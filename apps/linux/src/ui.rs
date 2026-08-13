//! Widget construction and rendering, kept apart from the model so `main.rs`
//! reads as state and behaviour rather than a wall of builders.
//!
//! Three rules run through this file. Lists are rebuilt only when a revision
//! counter changes, because progress events arrive many times a second and
//! tearing down rows under the pointer loses clicks. Every write back to a
//! stateful widget is guarded by a comparison, because setting a switch to the
//! value it already holds still fires its notify handler. And handlers are
//! connected exactly once, reading the live session from a shared cell rather
//! than being reconnected as the session changes.

use std::cell::Cell;
use std::rc::Rc;

use super::*;

/// The first of `candidates` the current theme actually has.
///
/// Ubuntu runs Yaru, not Adwaita, and themes disagree about which names exist —
/// Yaru has no `send-to-symbolic`, so naming it directly renders a broken-image
/// glyph. Bundling Adwaita does not help, because lookups go through the user's
/// theme first. Asking the theme what it has is the only reliable approach.
pub fn resolved_icon(candidates: &[&'static str]) -> &'static str {
    let last = candidates.last().copied().unwrap_or("image-missing");
    let Some(display) = gtk4::gdk::Display::default() else {
        return last;
    };
    let theme = gtk4::IconTheme::for_display(&display);
    for name in candidates {
        if theme.has_icon(name) {
            return name;
        }
    }
    tracing::warn!(?candidates, "no candidate icon exists in this theme");
    last
}

/// How long a banner stays before retiring itself.
const NOTICE_LIFETIME: std::time::Duration = std::time::Duration::from_secs(3);

/// The transfer card, used twice: once for what is arriving, once for what is
/// being sent. Identical apart from which buttons apply.
pub struct CardWidgets {
    pub root: gtk4::Box,
    title: gtk4::Label,
    summary: gtk4::Label,
    code_row: gtk4::Box,
    code: gtk4::Label,
    accept: gtk4::Button,
    decline: gtk4::Button,
    cancel: gtk4::Button,
    progress_area: gtk4::Box,
    progress: gtk4::ProgressBar,
    stats: gtk4::Label,
    files: gtk4::Box,
    files_scroll: gtk4::ScrolledWindow,
    file_rows: Vec<(gtk4::Label, gtk4::ProgressBar)>,
    file_count: usize,
    /// Read by the button handlers at click time. Connecting once and looking
    /// the session up here avoids reconnecting handlers on every render, which
    /// is what previously left Cancel wired to nothing.
    session: Rc<Cell<Option<SessionId>>>,
}

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
    pub visible_as: libadwaita::ActionRow,
    pub destination_row: libadwaita::ActionRow,
    pub launch_row: libadwaita::SwitchRow,
    pub sounds_row: libadwaita::SwitchRow,
    pub auto_off: libadwaita::ComboRow,
    pub trusted_group: libadwaita::PreferencesGroup,
    pub trusted_rows: Vec<libadwaita::ActionRow>,
    pub trusted_empty: libadwaita::ActionRow,
}

/// Style corrections applied over whatever theme is running.
///
/// Every rule here is libadwaita's own, restated. Ubuntu's Yaru stylesheet
/// flattens them — rows lose their internal padding, groups lose the gaps
/// between them, the status page icon shrinks from 128px to nothing — and the
/// result is a window that looks correct on Adwaita and cramped on Ubuntu.
/// Reasserting them at application priority keeps the layout identical under
/// both while leaving colours, fonts and shapes to the user's theme.
const STYLE: &str = "
/* Rows: the horizontal inset and vertical breathing room around the labels. */
row > box.header { margin-left: 12px; margin-right: 12px; border-spacing: 6px; min-height: 52px; }
row > box.header > box.title { margin-top: 8px; margin-bottom: 8px; border-spacing: 3px; }

/* Pages: outer margin and the gap between groups. */
preferencespage > scrolledwindow > viewport > clamp > box { margin: 24px 12px; border-spacing: 24px; }
preferencesgroup > box > box.header:not(.single-line) { margin-bottom: 6px; }

/* Empty states: a 128px icon, not a 16px one. */
statuspage > scrolledwindow > viewport > box { margin: 36px 12px; border-spacing: 36px; }
statuspage > scrolledwindow > viewport > box > clamp > box { border-spacing: 12px; }
statuspage > scrolledwindow > viewport > box > clamp > box > .icon { -gtk-icon-size: 128px; }
statuspage > scrolledwindow > viewport > box > clamp > box > .icon:not(:last-child) { margin-bottom: 24px; }

/* The switcher spaces icon from label with border-spacing, not margins. */
viewswitcher button.toggle > stack > box.wide { border-spacing: 8px; padding: 2px 14px; }
viewswitcher button.toggle > stack > box.narrow { border-spacing: 4px; }
";

/// Dialog colours as literals, picked for the current scheme.
///
/// libadwaita paints an alert with `@dialog_bg_color`, and Yaru never defines
/// that name — GTK then discards the whole declaration and the sheet is left
/// unpainted, so the page shows through the text. Defining the name in terms of
/// another named colour did not help either. Literal values cannot fail to
/// resolve, which is the entire point.
///
/// The values are libadwaita's own defaults for the two schemes.
fn dialog_style(dark: bool) -> String {
    let (background, foreground) = if dark {
        ("#383838", "#ffffff")
    } else {
        ("#fafafb", "rgba(0, 0, 0, 0.8)")
    };
    format!(
        "dialog.alert floating-sheet > sheet,
         dialog floating-sheet > sheet,
         dialog-host > dialog.alert sheet {{
           background-color: {background};
           color: {foreground};
           border-radius: 13px;
           box-shadow: 0 2px 6px rgba(0, 0, 0, 0.28), 0 8px 24px rgba(0, 0, 0, 0.42);
         }}
         dialog-host > dialog > dimming,
         dialog floating-sheet > dimming {{ background-color: rgba(0, 0, 0, 0.45); }}
         dialog.alert .message-area {{ padding: 24px 30px; border-spacing: 10px; }}
         dialog.alert .response-area > button {{ padding: 10px 14px; }}"
    )
}

fn install_style(display: &gtk4::gdk::Display) {
    // Repainted when the system switches between light and dark, since the
    // literals above are scheme-specific.
    let dialogs = gtk4::CssProvider::new();
    let manager = libadwaita::StyleManager::default();
    dialogs.load_from_string(&dialog_style(manager.is_dark()));
    gtk4::style_context_add_provider_for_display(
        display,
        &dialogs,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    {
        let dialogs = dialogs.clone();
        manager.connect_dark_notify(move |manager| {
            dialogs.load_from_string(&dialog_style(manager.is_dark()));
        });
    }

    let provider = gtk4::CssProvider::new();
    // A rule that fails to parse is dropped silently, which looks exactly like
    // a rule that matched nothing. Say which it was.
    provider.connect_parsing_error(|_, section, error| {
        tracing::error!(%error, section = %section.to_str(), "stylesheet rejected");
    });
    provider.load_from_string(STYLE);
    gtk4::style_context_add_provider_for_display(
        display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// Spacing for the hand-built pages, which get none of the preference page's
/// automatic rhythm.
const GROUP_GAP: i32 = 18;

fn add_group(page: &libadwaita::PreferencesPage, group: &libadwaita::PreferencesGroup) {
    page.add(group);
}

fn padded(widget: &impl IsA<gtk4::Widget>, amount: i32) {
    widget.as_ref().set_margin_top(amount);
    widget.as_ref().set_margin_bottom(amount);
    widget.as_ref().set_margin_start(amount);
    widget.as_ref().set_margin_end(amount);
}

fn build_card(sender: &ComponentSender<App>) -> CardWidgets {
    let session: Rc<Cell<Option<SessionId>>> = Rc::new(Cell::new(None));

    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    root.add_css_class("card");
    // Nothing is in flight at startup, and `update_view` does not run until the
    // first message arrives — so the card must be born hidden.
    root.set_visible(false);
    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    padded(&inner, 14);

    let heading = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    let titles = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    titles.set_hexpand(true);
    let title = gtk4::Label::builder().xalign(0.0).wrap(true).build();
    title.add_css_class("heading");
    let summary = gtk4::Label::builder().xalign(0.0).wrap(true).build();
    summary.add_css_class("dim-label");
    titles.append(&title);
    titles.append(&summary);
    heading.append(&titles);

    let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    buttons.set_valign(gtk4::Align::Start);
    let decline = gtk4::Button::with_label("Decline");
    let accept = gtk4::Button::with_label("Accept");
    accept.add_css_class("suggested-action");
    let cancel = gtk4::Button::with_label("Cancel");
    cancel.add_css_class("destructive-action");
    buttons.append(&decline);
    buttons.append(&accept);
    buttons.append(&cancel);
    heading.append(&buttons);
    inner.append(&heading);

    for (button, decision) in [
        (&accept, platform::Decision::Accept),
        (&decline, platform::Decision::Reject),
    ] {
        let sender = sender.clone();
        let session = session.clone();
        button.connect_clicked(move |_| {
            if let Some(session) = session.get() {
                sender.input(Msg::Consent { session, decision });
            }
        });
    }
    {
        let sender = sender.clone();
        let session = session.clone();
        cancel.connect_clicked(move |_| {
            if let Some(session) = session.get() {
                sender.input(Msg::Cancel(session));
            }
        });
    }

    let code_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let code_caption = gtk4::Label::new(Some("Code"));
    code_caption.add_css_class("dim-label");
    let code = gtk4::Label::new(None);
    code.add_css_class("title-2");
    code.add_css_class("numeric");
    let code_hint = gtk4::Label::builder()
        .label("must match the code shown on the phone")
        .wrap(true)
        .xalign(0.0)
        .build();
    code_hint.add_css_class("dim-label");
    code_row.append(&code_caption);
    code_row.append(&code);
    code_row.append(&code_hint);
    inner.append(&code_row);

    let progress_area = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    let progress = gtk4::ProgressBar::new();
    let stats = gtk4::Label::builder().xalign(0.0).build();
    stats.add_css_class("caption");
    stats.add_css_class("dim-label");
    progress_area.append(&progress);
    progress_area.append(&stats);
    inner.append(&progress_area);

    // Per-file rows scroll rather than growing the card past the window.
    let files = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    let files_scroll = gtk4::ScrolledWindow::builder()
        .max_content_height(140)
        .propagate_natural_height(true)
        .child(&files)
        .build();
    files_scroll.set_visible(false);
    inner.append(&files_scroll);
    root.append(&inner);

    CardWidgets {
        root,
        title,
        summary,
        code_row,
        code,
        accept,
        decline,
        cancel,
        progress_area,
        progress,
        stats,
        files,
        files_scroll,
        file_rows: Vec::new(),
        file_count: usize::MAX,
        session,
    }
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
        visible_as: settings_bits.visible_as,
        destination_row: settings_bits.destination_row,
        launch_row: settings_bits.launch_row,
        sounds_row: settings_bits.sounds_row,
        auto_off: settings_bits.auto_off,
        trusted_group: settings_bits.trusted_group,
        trusted_rows: Vec::new(),
        trusted_empty: settings_bits.trusted_empty,
    }
}

type TextCard = (gtk4::Box, gtk4::Label, gtk4::Label);
type EmptyState = (libadwaita::StatusPage, gtk4::Button);

/// Receiving: only what is arriving right now. Anything finished belongs to
/// History, so this page is empty most of the time and says so plainly.
fn build_receiving(
    sender: &ComponentSender<App>,
) -> (gtk4::Widget, CardWidgets, TextCard, EmptyState) {
    let page = gtk4::Box::new(gtk4::Orientation::Vertical, GROUP_GAP);
    padded(&page, 24);

    let incoming = build_card(sender);
    page.append(&incoming.root);

    // Text and links never touch the disk, so they stay on screen until
    // dismissed; the clipboard already holds the content.
    let text_card = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    text_card.add_css_class("card");
    text_card.set_visible(false);
    let text_inner = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    padded(&text_inner, 14);
    let text_body = gtk4::Box::new(gtk4::Orientation::Vertical, 3);
    text_body.set_hexpand(true);
    let text_card_title = gtk4::Label::builder().xalign(0.0).build();
    text_card_title.add_css_class("heading");
    let text_card_body = gtk4::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .selectable(true)
        .build();
    text_card_body.add_css_class("dim-label");
    text_body.append(&text_card_title);
    text_body.append(&text_card_body);
    text_inner.append(&text_body);
    let dismiss_text = gtk4::Button::from_icon_name("window-close-symbolic");
    dismiss_text.set_valign(gtk4::Align::Center);
    dismiss_text.add_css_class("flat");
    {
        let sender = sender.clone();
        dismiss_text.connect_clicked(move |_| sender.input(Msg::DismissText));
    }
    text_inner.append(&dismiss_text);
    text_card.append(&text_inner);
    page.append(&text_card);

    let empty_action = gtk4::Button::with_label("Turn on receiving");
    empty_action.add_css_class("suggested-action");
    empty_action.add_css_class("pill");
    empty_action.set_halign(gtk4::Align::Center);
    {
        let sender = sender.clone();
        empty_action.connect_clicked(move |_| sender.input(Msg::SetReceiving(true)));
    }
    let empty_state = libadwaita::StatusPage::builder()
        .icon_name("folder-download-symbolic")
        .vexpand(true)
        .child(&empty_action)
        .build();
    page.append(&empty_state);

    let scroller = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .child(&page)
        .build();

    (
        scroller.upcast(),
        incoming,
        (text_card, text_card_title, text_card_body),
        (empty_state, empty_action),
    )
}

struct HistoryBits {
    filter: gtk4::DropDown,
    list: gtk4::ListBox,
    empty: libadwaita::StatusPage,
    scroll: gtk4::ScrolledWindow,
}

fn build_history(sender: &ComponentSender<App>) -> (gtk4::Widget, HistoryBits) {
    let page = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    padded(&page, 16);

    let controls = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let search = gtk4::SearchEntry::builder()
        .placeholder_text("Search name, extension, link or sender")
        .hexpand(true)
        .build();
    {
        let sender = sender.clone();
        search.connect_search_changed(move |entry| {
            sender.input(Msg::Search(entry.text().to_string()));
        });
    }
    let filter = gtk4::DropDown::from_strings(&["All"]);
    let clear = gtk4::Button::with_label("Clear");
    clear.add_css_class("destructive-action");
    {
        let sender = sender.clone();
        clear.connect_clicked(move |_| sender.input(Msg::ClearHistory));
    }
    controls.append(&search);
    controls.append(&filter);
    controls.append(&clear);
    page.append(&controls);

    let list = gtk4::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.set_valign(gtk4::Align::Start);
    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vexpand(true)
        .child(&list)
        .build();
    page.append(&scroll);

    let empty = libadwaita::StatusPage::builder()
        .icon_name("document-open-recent-symbolic")
        .title("Nothing yet")
        .description("Transfers appear here once they finish.")
        .vexpand(true)
        .build();
    page.append(&empty);

    (
        page.upcast(),
        HistoryBits {
            filter,
            list,
            empty,
            scroll,
        },
    )
}

struct SendBits {
    discovering_row: libadwaita::SwitchRow,
    staged_headline: gtk4::Label,
    staged_detail: gtk4::Label,
    staged_area: gtk4::Box,
    pick_area: gtk4::Box,
    text_entry: libadwaita::EntryRow,
    compose_group: libadwaita::PreferencesGroup,
    endpoints_group: libadwaita::PreferencesGroup,
    no_devices: libadwaita::ActionRow,
    send_group: libadwaita::PreferencesGroup,
    send_button: gtk4::Button,
    outgoing_group: libadwaita::PreferencesGroup,
    retry_group: libadwaita::PreferencesGroup,
}

fn build_send(sender: &ComponentSender<App>) -> (gtk4::Widget, CardWidgets, SendBits) {
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
    {
        let sender = sender.clone();
        discovering_row.connect_active_notify(move |row| {
            sender.input(Msg::SetDiscovering(row.is_active()));
        });
    }
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

struct SettingsBits {
    receiving_row: libadwaita::SwitchRow,
    visible_as: libadwaita::ActionRow,
    destination_row: libadwaita::ActionRow,
    launch_row: libadwaita::SwitchRow,
    sounds_row: libadwaita::SwitchRow,
    auto_off: libadwaita::ComboRow,
    trusted_group: libadwaita::PreferencesGroup,
    trusted_empty: libadwaita::ActionRow,
}

fn build_settings(sender: &ComponentSender<App>, model: &App) -> (gtk4::Widget, SettingsBits) {
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

/// Walk a widget's CSS nodes, so a styling problem can be diagnosed from a log
/// rather than by guessing at selectors.
#[allow(dead_code)]
fn log_tree(widget: &gtk4::Widget, depth: usize) {
    if depth > 4 {
        return;
    }
    let classes = widget.css_classes().join(".");
    tracing::info!(
        "{:indent$}{} [{}] {}",
        "",
        widget.css_name(),
        classes,
        widget.type_(),
        indent = depth * 2
    );
    let mut child = widget.first_child();
    while let Some(node) = child {
        log_tree(&node, depth + 1);
        child = node.next_sibling();
    }
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

/// Push model state into the widgets.
pub fn render(model: &App, widgets: &mut AppWidgets, sender: &ComponentSender<App>) {
    match &model.notice {
        Some(text) => {
            widgets.notice_banner.set_title(text);
            widgets.notice_banner.set_revealed(true);
            // Start the expiry once per notice, not on every render.
            if widgets.notice_scheduled != model.notice_id {
                widgets.notice_scheduled = model.notice_id;
                let sender = sender.clone();
                let id = model.notice_id;
                gtk4::glib::timeout_add_local_once(NOTICE_LIFETIME, move || {
                    sender.input(Msg::ExpireNotice(id));
                });
            }
        }
        None => widgets.notice_banner.set_revealed(false),
    }

    if widgets.receiving_row.is_active() != model.receiving {
        widgets.receiving_row.set_active(model.receiving);
    }
    if widgets.discovering_row.is_active() != model.discovering {
        widgets.discovering_row.set_active(model.discovering);
    }
    if widgets.launch_row.is_active() != model.prefs.launch_at_login {
        widgets.launch_row.set_active(model.prefs.launch_at_login);
    }
    if widgets.sounds_row.is_active() != model.prefs.play_sounds {
        widgets.sounds_row.set_active(model.prefs.play_sounds);
    }
    let auto_index = AUTO_OFF_CHOICES
        .iter()
        .position(|minutes| *minutes == model.prefs.auto_off_minutes)
        .unwrap_or(0) as u32;
    if widgets.auto_off.selected() != auto_index {
        widgets.auto_off.set_selected(auto_index);
    }
    widgets.visible_as.set_subtitle(&model.device_name);
    widgets.visible_as.set_visible(model.receiving);
    widgets
        .destination_row
        .set_subtitle(&model.destination.display().to_string());

    // The same transfer never belongs to both cards.
    let incoming = model.active.as_ref().filter(|active| !active.outgoing);
    let outgoing = model.active.as_ref().filter(|active| active.outgoing);
    render_card(&mut widgets.incoming, incoming);
    render_card(&mut widgets.outgoing, outgoing);

    match &model.received_text {
        Some(text) => {
            widgets.text_card.set_visible(true);
            widgets.text_card_title.set_label(if text.kind == "link" {
                "Link copied to the clipboard"
            } else {
                "Text copied to the clipboard"
            });
            widgets.text_card_body.set_label(&text.content);
        }
        None => widgets.text_card.set_visible(false),
    }

    let idle = incoming.is_none() && model.received_text.is_none();
    widgets.empty_state.set_visible(idle);
    widgets.empty_state.set_title(if model.receiving {
        "Ready to receive"
    } else {
        "Receiving is off"
    });
    widgets
        .empty_state
        .set_description(Some(&if model.receiving {
            format!(
                "On the phone: pick files, then Share → Quick Share → “{}”.",
                model.device_name
            )
        } else {
            "Turn receiving on to accept files from nearby Android devices.".to_string()
        }));
    widgets.empty_action.set_visible(!model.receiving);

    if widgets.history_revision != model.history_revision {
        widgets.history_revision = model.history_revision;
        render_filter(model, widgets, sender);
        render_history(model, widgets, sender);
    }

    // While something is going out, the composer and device list would only
    // invite a second send that cannot start yet.
    let sending = outgoing.is_some();
    widgets
        .retry_group
        .set_visible(model.retry_available && !sending);
    widgets.outgoing_group.set_visible(sending);
    widgets.compose_group.set_visible(!sending);
    widgets.endpoints_group.set_visible(!sending);
    widgets.send_group.set_visible(!sending);

    match &model.staged {
        Some(staged) => {
            widgets.staged_area.set_visible(true);
            widgets.pick_area.set_visible(false);
            widgets.staged_headline.set_label(&staged.headline());
            widgets.staged_detail.set_label(&staged.detail());
            widgets
                .text_entry
                .set_visible(matches!(staged, Staged::Text(_)));
            // Text can arrive by drop as well as by typing, so the entry has to
            // be told what it now holds.
            if let Staged::Text(text) = staged {
                if widgets.text_entry.text() != *text {
                    widgets.text_entry.set_text(text);
                }
            }
        }
        None => {
            widgets.staged_area.set_visible(false);
            widgets.pick_area.set_visible(true);
            widgets.text_entry.set_visible(true);
            if !widgets.text_entry.text().is_empty() {
                widgets.text_entry.set_text("");
            }
        }
    }
    widgets
        .send_button
        .set_sensitive(model.staged.is_some() && model.selected_endpoint.is_some());

    render_endpoints(model, widgets, sender);
    render_trusted(model, widgets, sender);
}

fn render_card(widgets: &mut CardWidgets, active: Option<&transfer::Active>) {
    let Some(active) = active else {
        widgets.root.set_visible(false);
        widgets.session.set(None);
        return;
    };

    widgets.root.set_visible(true);
    widgets.session.set(Some(active.session));
    widgets.title.set_label(&active.title());
    widgets.summary.set_label(&active.summary());

    // Incoming and still waiting for an answer: the only case with Accept.
    let deciding = !active.running && !active.outgoing;
    widgets.accept.set_visible(deciding);
    widgets.decline.set_visible(deciding);
    // Everything else that is live can be called off — including an outbound
    // transfer the phone has not answered yet, which would otherwise block the
    // next send until it timed out.
    widgets.cancel.set_visible(!deciding);

    // The code exists to be compared before agreeing; once bytes are moving it
    // has already done its job and only takes up room.
    widgets
        .code_row
        .set_visible(!active.running && !active.token.is_empty());
    widgets.code.set_label(&active.token);

    widgets
        .progress_area
        .set_visible(active.running && active.total_bytes > 0);
    widgets.progress.set_fraction(active.fraction());
    let mut stats = format!(
        "{} of {}",
        format::bytes(active.bytes),
        format::bytes(active.total_bytes)
    );
    if let Some(rate) = active.rate() {
        stats.push_str(&format!(" · {}", format::rate(rate)));
        if let Some(left) = active.seconds_remaining() {
            stats.push_str(&format!(" · {}", format::remaining(left)));
        }
    }
    widgets.stats.set_label(&stats);

    // Rebuild file rows only when the set of files changed.
    if widgets.file_count != active.files.len() {
        widgets.file_count = active.files.len();
        while let Some(child) = widgets.files.first_child() {
            widgets.files.remove(&child);
        }
        widgets.file_rows.clear();
        for _ in &active.files {
            let row = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
            let label = gtk4::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk4::pango::EllipsizeMode::Middle)
                .build();
            label.add_css_class("caption");
            let bar = gtk4::ProgressBar::new();
            row.append(&label);
            row.append(&bar);
            widgets.files.append(&row);
            widgets.file_rows.push((label, bar));
        }
    }
    for ((label, bar), file) in widgets.file_rows.iter().zip(&active.files) {
        label.set_label(&format!("{} · {}", file.name, format::bytes(file.size)));
        bar.set_fraction(file.fraction());
        bar.set_visible(active.files.len() > 1 && active.running);
    }
    let show_files = !active.files.is_empty() && active.text_preview.is_none();
    widgets.files.set_visible(show_files);
    widgets.files_scroll.set_visible(show_files);
}

fn render_filter(model: &App, widgets: &mut AppWidgets, sender: &ComponentSender<App>) {
    let available = model.available_categories();
    if available == widgets.filter_options {
        return;
    }
    // Replacing the model fires `selected-notify`; without dropping the handler
    // first, that would reset the chosen category whenever the list changed.
    if let Some(handler) = widgets.filter_handler.take() {
        widgets.filter.disconnect(handler);
    }
    let titles: Vec<&str> = available.iter().map(|c| c.title()).collect();
    widgets
        .filter
        .set_model(Some(&gtk4::StringList::new(&titles)));
    let selected = available
        .iter()
        .position(|category| *category == model.category)
        .unwrap_or(0) as u32;
    widgets.filter.set_selected(selected);
    widgets.filter_options = available;

    let sender = sender.clone();
    let options = widgets.filter_options.clone();
    widgets.filter_handler = Some(widgets.filter.connect_selected_notify(move |dropdown| {
        if let Some(category) = options.get(dropdown.selected() as usize) {
            sender.input(Msg::Filter(*category));
        }
    }));
}

fn render_history(model: &App, widgets: &mut AppWidgets, sender: &ComponentSender<App>) {
    while let Some(row) = widgets.history_list.first_child() {
        widgets.history_list.remove(&row);
    }

    let visible = model.visible_history();
    let nothing_at_all = model.history.is_empty();
    widgets.history_scroll.set_visible(!visible.is_empty());
    widgets.history_empty.set_visible(visible.is_empty());
    widgets.history_empty.set_title(if nothing_at_all {
        "Nothing yet"
    } else {
        "Nothing matches"
    });
    widgets.history_empty.set_description(Some(if nothing_at_all {
        "Transfers appear here once they finish."
    } else {
        "Try a different search or category. Names, extensions, links and senders are all searched."
    }));

    for entry in visible {
        let row = libadwaita::ActionRow::builder()
            .title(glib_escape(&entry.summary()))
            .subtitle(format!(
                "{} · {}",
                entry.peer,
                entry
                    .date
                    .with_timezone(&chrono::Local)
                    .format("%d %b %H:%M")
            ))
            .build();
        row.add_prefix(&gtk4::Image::from_icon_name(entry.icon()));

        if entry.has_file() {
            let reveal = gtk4::Button::from_icon_name("folder-open-symbolic");
            reveal.set_tooltip_text(Some("Show in file manager"));
            reveal.set_valign(gtk4::Align::Center);
            reveal.add_css_class("flat");
            let sender = sender.clone();
            let path = PathBuf::from(entry.paths[0].clone());
            reveal.connect_clicked(move |_| sender.input(Msg::Reveal(path.clone())));
            row.add_suffix(&reveal);
        } else if let Some(content) = entry.content.clone() {
            let copy = gtk4::Button::from_icon_name("edit-copy-symbolic");
            copy.set_tooltip_text(Some("Copy"));
            copy.set_valign(gtk4::Align::Center);
            copy.add_css_class("flat");
            let sender = sender.clone();
            copy.connect_clicked(move |_| sender.input(Msg::CopyText(content.clone())));
            row.add_suffix(&copy);
        }

        // "Remove", not "Delete": the file it refers to stays where it was
        // saved.
        let remove = gtk4::Button::from_icon_name("list-remove-symbolic");
        remove.set_tooltip_text(Some("Remove from history"));
        remove.set_valign(gtk4::Align::Center);
        remove.add_css_class("flat");
        let sender = sender.clone();
        let id = entry.id;
        remove.connect_clicked(move |_| sender.input(Msg::RemoveEntry(id)));
        row.add_suffix(&remove);

        widgets.history_list.append(&row);
    }
}

fn render_endpoints(model: &App, widgets: &mut AppWidgets, sender: &ComponentSender<App>) {
    widgets.endpoint_rows.retain(|id, row| {
        let still_here = model.endpoints.iter().any(|e| &e.id == id);
        if !still_here {
            widgets.endpoints_group.remove(row);
        }
        still_here
    });

    for endpoint in &model.endpoints {
        if widgets.endpoint_rows.contains_key(&endpoint.id) {
            continue;
        }
        let check = gtk4::CheckButton::builder()
            .valign(gtk4::Align::Center)
            .build();
        match &widgets.endpoint_group_leader {
            Some(leader) => check.set_group(Some(leader)),
            None => widgets.endpoint_group_leader = Some(check.clone()),
        }
        {
            let sender = sender.clone();
            let id = endpoint.id.clone();
            check.connect_toggled(move |button| {
                if button.is_active() {
                    sender.input(Msg::SelectEndpoint(id.clone()));
                }
            });
        }

        let row = libadwaita::ActionRow::builder()
            .title(glib_escape(&endpoint.name))
            .subtitle(&endpoint.kind)
            .activatable_widget(&check)
            .build();
        row.add_prefix(&gtk4::Image::from_icon_name(endpoint.icon()));
        row.add_suffix(&check);
        widgets.endpoints_group.add(&row);
        widgets.endpoint_rows.insert(endpoint.id.clone(), row);
    }

    widgets.no_devices.set_visible(model.endpoints.is_empty());
}

fn render_trusted(model: &App, widgets: &mut AppWidgets, sender: &ComponentSender<App>) {
    if widgets.trusted_rows.len() == model.prefs.trusted_devices.len() {
        return;
    }
    for row in widgets.trusted_rows.drain(..) {
        widgets.trusted_group.remove(&row);
    }
    for device in &model.prefs.trusted_devices {
        let row = libadwaita::ActionRow::builder()
            .title(glib_escape(device))
            .build();
        let revoke = gtk4::Button::with_label("Revoke");
        revoke.set_valign(gtk4::Align::Center);
        let sender = sender.clone();
        let device = device.clone();
        revoke.connect_clicked(move |_| sender.input(Msg::Revoke(device.clone())));
        row.add_suffix(&revoke);
        widgets.trusted_group.add(&row);
        widgets.trusted_rows.push(row);
    }
    widgets
        .trusted_empty
        .set_visible(model.prefs.trusted_devices.is_empty());
}

/// Adwaita rows treat their title as Pango markup, so a filename containing
/// `&` or `<` would either vanish or break the row.
fn glib_escape(text: &str) -> String {
    gtk4::glib::markup_escape_text(text).to_string()
}
