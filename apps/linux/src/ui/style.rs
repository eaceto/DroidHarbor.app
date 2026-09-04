//! The stylesheet: corrections applied over whatever theme is running, plus
//! the app's own classes.

/// Style corrections applied over whatever theme is running.
///
/// Every layout rule here is libadwaita's own, restated. Ubuntu's Yaru
/// stylesheet flattens them — rows lose their internal padding, groups lose
/// the gaps between them, the status page icon shrinks from 128px to nothing —
/// and the result is a window that looks correct on Adwaita and cramped on
/// Ubuntu. Reasserting them at application priority keeps the layout identical
/// under both while leaving colours, fonts and shapes to the user's theme.
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

/* The brand teal, matching the macOS app's tint. Literal rather than
   @accent_bg_color for the same reason as dialog_style below: Yaru does not
   define Adwaita's named colours, and a declaration naming one is dropped
   whole. alpha() keeps every use legible on both schemes. */

/* The 4-digit code set as the ticket it is: matching it against the phone is
   the whole security ritual, so it is the one typographic flourish. */
.code-ticket { background-color: alpha(#30b0c7, 0.15); border-radius: 8px; padding: 2px 10px; }

/* Teal-washed only while a decision is pending — the app's one urgent
   surface. Once bytes move the card returns to neutral. */
.card.consent { background-color: alpha(#30b0c7, 0.08); border: 1px solid alpha(#30b0c7, 0.35); }

/* Empty states carry the identity quietly instead of reading as gray voids. */
statuspage.empty-accent .icon { color: alpha(#30b0c7, 0.8); }
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

pub(super) fn install_style(display: &gtk4::gdk::Display) {
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
