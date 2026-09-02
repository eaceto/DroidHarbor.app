//! Receiving: only what is arriving right now. Anything finished belongs to
//! History, so this page is empty most of the time and says so plainly.

use gtk4::prelude::*;
use relm4::ComponentSender;

use crate::app::{App, Msg};

use super::card::{build_card, CardWidgets};
use super::{padded, GROUP_GAP};

pub(super) type TextCard = (gtk4::Box, gtk4::Label, gtk4::Label);
pub(super) type EmptyState = (libadwaita::StatusPage, gtk4::Button);

pub(super) fn build_receiving(
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
    empty_state.add_css_class("empty-accent");
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
