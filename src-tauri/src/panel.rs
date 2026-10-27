//! macOS only: turns the history window into a non-activating panel, the
//! kind Spotlight uses. An ordinary window can't appear over another app's
//! full-screen Space, and activating our app to show it would switch the
//! user out of that Space.

use tauri::{AppHandle, WebviewWindow};
use tauri_nspanel::{tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt};

tauri_panel! {
    panel!(ClipPanel {
        config: {
            // Needed so the search box can take keyboard input.
            can_become_key_window: true,
            is_floating_panel: true
        }
    })
}

pub fn setup(win: &WebviewWindow) -> tauri::Result<()> {
    let panel = win.to_panel::<ClipPanel>()?;
    panel.set_level(PanelLevel::Floating.value());
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .full_screen_auxiliary()
            .can_join_all_spaces()
            .value(),
    );
    if let Err(e) = panel.add_style_mask(StyleMask::empty().nonactivating_panel().value()) {
        eprintln!("could not make the panel non-activating: {e:?}");
    }
    Ok(())
}

/// Shows the panel and gives it keyboard focus without activating the app.
/// Returns false if the window was never converted, so the caller can fall
/// back to showing it as a normal window.
pub fn show(app: &AppHandle) -> bool {
    match app.get_webview_panel("main") {
        Ok(panel) => {
            panel.show_and_make_key();
            true
        }
        Err(_) => false,
    }
}
