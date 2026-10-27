mod db;
#[cfg(target_os = "macos")]
mod panel;
mod platform;
mod shell;
mod watcher;

use db::{Clip, Store};
use tauri::{Manager, State};

/// Unpinned clips kept before the oldest are dropped.
const MAX_ITEMS: usize = 500;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
fn list_clips(store: State<Store>, query: String, limit: Option<usize>) -> Result<Vec<Clip>, String> {
    store.list(&query, limit.unwrap_or(200)).map_err(err)
}

#[tauri::command]
fn set_pinned(store: State<Store>, id: i64, pinned: bool) -> Result<(), String> {
    store.set_pinned(id, pinned).map_err(err)
}

#[tauri::command]
fn delete_clip(store: State<Store>, id: i64) -> Result<(), String> {
    store.delete(id).map_err(err)
}

#[tauri::command]
fn copy_clip(store: State<Store>, id: i64) -> Result<(), String> {
    let clip = store.get(id).map_err(err)?.ok_or("clip not found")?;
    watcher::write_back(&clip)
}

#[tauri::command]
fn hide_window(app: tauri::AppHandle) {
    shell::hide(&app);
}

#[tauri::command]
fn clear_history(store: State<Store>) -> Result<(), String> {
    store.clear_unpinned().map_err(err)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        // Must be registered first: a second launch just opens the window.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| shell::show(app)))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![shell::HIDDEN_ARG]),
        ));
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    builder
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            app.manage(Store::open(&data_dir, MAX_ITEMS)?);
            watcher::spawn(app.handle().clone());
            shell::setup(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_clips,
            set_pinned,
            delete_clip,
            copy_clip,
            hide_window,
            clear_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
