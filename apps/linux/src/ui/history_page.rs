//! The searchable, filterable record of past transfers.

use gtk4::prelude::*;
use libadwaita::prelude::*;
use relm4::ComponentSender;

use crate::app::{App, Msg};
use crate::history;

use super::{glib_escape, padded, AppWidgets};

pub(super) struct HistoryBits {
    pub filter: gtk4::DropDown,
    pub list: gtk4::ListBox,
    pub empty: libadwaita::StatusPage,
    pub scroll: gtk4::ScrolledWindow,
}

pub(super) fn build_history(sender: &ComponentSender<App>) -> (gtk4::Widget, HistoryBits) {
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
    empty.add_css_class("empty-accent");
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

pub(super) fn render_filter(model: &App, widgets: &mut AppWidgets, sender: &ComponentSender<App>) {
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

pub(super) fn render_history(model: &App, widgets: &mut AppWidgets, sender: &ComponentSender<App>) {
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
            // Resolved, not taken verbatim: entries from older builds hold
            // bare names, which the file manager cannot be pointed at.
            let path = history::resolve_path(&entry.paths[0], &model.destination);
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
