mod db;
mod platform;
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
fn clear_history(store: State<Store>) -> Result<(), String> {
    store.clear_unpinned().map_err(err)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            app.manage(Store::open(&data_dir, MAX_ITEMS)?);
            watcher::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_clips,
            set_pinned,
            delete_clip,
            copy_clip,
            clear_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
