//! Widget construction and rendering, kept apart from the model so `app`
//! reads as state and behaviour rather than a wall of builders. One module
//! per page, plus the pieces they share: the transfer card, the stylesheet,
//! the dialogs, and the render pass that pushes model state into widgets.
//!
//! Three rules run through these modules. Lists are rebuilt only when a
//! revision counter changes, because progress events arrive many times a
//! second and tearing down rows under the pointer loses clicks. Every write
//! back to a stateful widget is guarded by a comparison, because setting a
//! switch to the value it already holds still fires its notify handler. And
//! handlers are connected exactly once, reading the live session from a
//! shared cell rather than being reconnected as the session changes.

mod card;
mod dialogs;
mod history_page;
mod receiving;
mod render;
mod send;
mod settings;
mod style;
mod widgets;

pub use dialogs::{confirm_clear_history, present_about, present_onboarding};
pub use render::render;
pub use widgets::{build, AppWidgets};

use gtk4::prelude::*;

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

/// Spacing for the hand-built pages, which get none of the preference page's
/// automatic rhythm.
const GROUP_GAP: i32 = 18;

fn add_group(page: &libadwaita::PreferencesPage, group: &libadwaita::PreferencesGroup) {
    use libadwaita::prelude::*;
    page.add(group);
}

fn padded(widget: &impl IsA<gtk4::Widget>, amount: i32) {
    widget.as_ref().set_margin_top(amount);
    widget.as_ref().set_margin_bottom(amount);
    widget.as_ref().set_margin_start(amount);
    widget.as_ref().set_margin_end(amount);
}

/// Adwaita rows treat their title as Pango markup, so a filename containing
/// `&` or `<` would either vanish or break the row.
fn glib_escape(text: &str) -> String {
    gtk4::glib::markup_escape_text(text).to_string()
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
