//! The transfer card, used twice: once for what is arriving, once for what is
//! being sent. Identical apart from which buttons apply.

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use relm4::ComponentSender;

use dh_domain::SessionId;

use crate::app::{App, Msg};
use crate::{format, platform, transfer};

use super::padded;

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

pub(super) fn build_card(sender: &ComponentSender<App>) -> CardWidgets {
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

    // The ticket carries the "code" idea on its own; a caption saying "Code"
    // next to it was saying it twice.
    let code_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    let code = gtk4::Label::new(None);
    code.add_css_class("title-2");
    code.add_css_class("numeric");
    code.add_css_class("code-ticket");
    let code_hint = gtk4::Label::builder()
        .label("must match the code shown on the phone")
        .wrap(true)
        .xalign(0.0)
        .build();
    code_hint.add_css_class("dim-label");
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

pub(super) fn render_card(widgets: &mut CardWidgets, active: Option<&transfer::Active>) {
    let Some(active) = active else {
        widgets.root.set_visible(false);
        widgets.session.set(None);
        return;
    };

    widgets.root.set_visible(true);
    widgets.session.set(Some(active.session));
    widgets.title.set_label(&active.title());
    widgets.summary.set_label(&active.summary());

    // Tinted only while a consent is pending, on either side of the wire;
    // an ordinary working card once the transfer runs.
    if active.running {
        widgets.root.remove_css_class("consent");
    } else {
        widgets.root.add_css_class("consent");
    }

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
