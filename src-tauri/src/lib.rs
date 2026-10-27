mod db;
#[cfg(target_os = "macos")]
mod panel;
mod platform;
mod settings;
mod shell;
mod watcher;

use db::{Clip, Store};
use serde::Serialize;
use settings::{Settings, SettingsStore};
use tauri::{AppHandle, Emitter, Manager, State};

/// Event sent to the UI after settings change, so it can restyle itself.
const SETTINGS_CHANGED: &str = "settings-changed";

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

/// Settings as the UI sees them, plus the OS-held launch-at-login flag.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsView {
    #[serde(flatten)]
    settings: Settings,
    launch_at_login: bool,
}

fn settings_view(app: &AppHandle) -> SettingsView {
    SettingsView {
        settings: app.state::<SettingsStore>().get(),
        launch_at_login: shell::launch_at_login(app),
    }
}

#[tauri::command]
fn get_settings(app: AppHandle) -> SettingsView {
    settings_view(&app)
}

/// Validates and applies new settings, then saves them. Anything that
/// can't be applied (e.g. a shortcut another app owns) is rejected before
/// saving, so the stored settings always match what's in effect.
#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<SettingsView, String> {
    settings.validate()?;
    let old = app.state::<SettingsStore>().get();
    shell::change_shortcut(&app, &old.shortcut, &settings.shortcut)?;
    if let Err(e) = app.state::<SettingsStore>().save(settings.clone()) {
        let _ = shell::change_shortcut(&app, &settings.shortcut, &old.shortcut);
        return Err(e);
    }
    if settings.max_items != old.max_items {
        app.state::<Store>().set_max_items(settings.max_items).map_err(err)?;
        let _ = app.emit(watcher::CLIPS_CHANGED, ());
    }
    let view = settings_view(&app);
    let _ = app.emit(SETTINGS_CHANGED, &view);
    Ok(view)
}

#[tauri::command]
fn set_launch_at_login(app: AppHandle, enabled: bool) -> Result<SettingsView, String> {
    shell::set_launch_at_login(&app, enabled)?;
    Ok(settings_view(&app))
}

#[tauri::command]
fn pause_shortcut(app: AppHandle, paused: bool) {
    shell::set_shortcut_paused(&app, paused);
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
            let settings = SettingsStore::load(&data_dir);
            app.manage(Store::open(&data_dir, settings.get().max_items)?);
            app.manage(settings);
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
            clear_history,
            get_settings,
            save_settings,
            set_launch_at_login,
            pause_shortcut
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
