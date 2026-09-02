//! Push model state into the widgets.

use gtk4::prelude::*;
use libadwaita::prelude::*;
use relm4::ComponentSender;

use crate::app::{App, Msg, Staged, AUTO_OFF_CHOICES};

use super::card::render_card;
use super::history_page::{render_filter, render_history};
use super::{glib_escape, AppWidgets};

/// How long a banner stays before retiring itself.
const NOTICE_LIFETIME: std::time::Duration = std::time::Duration::from_secs(3);

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
    // The name is always here: while receiving it is what phones can see,
    // and while off it is what they would see, so a rename reads back
    // against the same row either way.
    widgets.visible_as.set_subtitle(&model.device_name);
    widgets.visible_as.set_title(if model.receiving {
        "Visible as"
    } else {
        "Will appear as"
    });
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
