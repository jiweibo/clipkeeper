//! Everything around the history window: showing and hiding it, the global
//! shortcut, the tray icon and its menu, and launch at login.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Emitter, Manager, WebviewWindow, Wry};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::settings::{shortcut_label, SettingsStore};

/// Argument passed when launched at login, so the app starts in the tray.
pub const HIDDEN_ARG: &str = "--hidden";

/// Event sent to the UI each time the window is shown, so it can reset
/// the search and focus the input.
const WINDOW_SHOWN: &str = "window-shown";
/// Event asking the UI to switch to the settings page.
const OPEN_SETTINGS: &str = "open-settings";

/// Tray menu items whose text or check mark follows the settings.
struct TrayItems {
    show: MenuItem<Wry>,
    autostart: CheckMenuItem<Wry>,
}

/// When the window was last hidden. Pressing the shortcut or clicking the
/// tray icon can take focus from the window first, which hides it; the
/// toggle that follows must not reopen it.
static LAST_HIDDEN: Mutex<Option<Instant>> = Mutex::new(None);
const REOPEN_GUARD: Duration = Duration::from_millis(300);

fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("main")
}

pub fn show(app: &AppHandle) {
    let Some(win) = main_window(app) else { return };
    let _ = win.center();
    #[cfg(target_os = "macos")]
    let shown = crate::panel::show(app);
    #[cfg(not(target_os = "macos"))]
    let shown = false;
    if !shown {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
    let _ = app.emit(WINDOW_SHOWN, ());
}

pub fn hide(app: &AppHandle) {
    let Some(win) = main_window(app) else { return };
    if win.is_visible().unwrap_or(false) {
        *LAST_HIDDEN.lock().unwrap() = Some(Instant::now());
    }
    let _ = win.hide();
}

pub fn toggle(app: &AppHandle) {
    let visible = main_window(app)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    let just_hidden = LAST_HIDDEN
        .lock()
        .unwrap()
        .is_some_and(|t| t.elapsed() < REOPEN_GUARD);
    if visible {
        hide(app);
    } else if !just_hidden {
        show(app);
    }
}

pub fn setup(app: &mut App) -> tauri::Result<()> {
    // Live in the menu bar only, without a Dock icon.
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    let shortcut = app.state::<SettingsStore>().get().shortcut;
    if let Err(e) = register_shortcut(app.handle(), &shortcut) {
        // Another app may own the shortcut; the tray icon still works.
        eprintln!("{e}");
    }
    build_tray(app)?;

    let win = main_window(app.handle()).expect("main window is defined in tauri.conf.json");
    #[cfg(target_os = "macos")]
    if let Err(e) = crate::panel::setup(&win) {
        eprintln!("could not convert window to panel: {e}");
    }
    let handle = app.handle().clone();
    win.on_window_event(move |event| match event {
        // Closing just hides; quitting is in the tray menu.
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide(&handle);
        }
        // Behave like a popup: clicking elsewhere dismisses it.
        tauri::WindowEvent::Focused(false) => hide(&handle),
        _ => {}
    });

    if !std::env::args().any(|a| a == HIDDEN_ARG) {
        show(app.handle());
    }
    Ok(())
}

fn register_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    let parsed: Shortcut = shortcut
        .parse()
        .map_err(|_| format!("无法识别的快捷键：{shortcut}"))?;
    app.global_shortcut()
        .on_shortcut(parsed, |app, _, event| {
            if event.state == ShortcutState::Pressed {
                toggle(app);
            }
        })
        .map_err(|_| "这个快捷键已被系统或其他应用占用，请换一个".to_string())
}

/// Switches the global shortcut. The new one is registered before the old
/// one is released, so a failed change leaves the old shortcut working.
pub fn change_shortcut(app: &AppHandle, old: &str, new: &str) -> Result<(), String> {
    if old == new {
        return Ok(());
    }
    register_shortcut(app, new)?;
    if let Ok(parsed) = old.parse::<Shortcut>() {
        let _ = app.global_shortcut().unregister(parsed);
    }
    refresh_tray(app);
    Ok(())
}

/// Turns the global shortcut off while the user records a new one, so
/// pressing the current combination doesn't toggle the window.
pub fn set_shortcut_paused(app: &AppHandle, paused: bool) {
    let _ = app.global_shortcut().unregister_all();
    if !paused {
        let shortcut = app.state::<SettingsStore>().get().shortcut;
        if let Err(e) = register_shortcut(app, &shortcut) {
            eprintln!("{e}");
        }
    }
}

pub fn launch_at_login(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

pub fn set_launch_at_login(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    refresh_tray(app);
    result.map_err(|e| format!("修改开机启动失败：{e}"))
}

/// Brings the tray menu in line with the current settings.
fn refresh_tray(app: &AppHandle) {
    let Some(items) = app.try_state::<TrayItems>() else { return };
    let shortcut = app.state::<SettingsStore>().get().shortcut;
    let _ = items.show.set_text(show_item_text(&shortcut));
    let _ = items.autostart.set_checked(launch_at_login(app));
}

fn show_item_text(shortcut: &str) -> String {
    format!("显示剪贴板历史    {}", shortcut_label(shortcut, cfg!(target_os = "macos")))
}

fn build_tray(app: &mut App) -> tauri::Result<()> {
    let autostart_on = launch_at_login(app.handle());
    let shortcut = app.state::<SettingsStore>().get().shortcut;
    let show_item = MenuItem::with_id(app, "show", show_item_text(&shortcut), true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)?;
    let autostart_item =
        CheckMenuItem::with_id(app, "autostart", "开机自动启动", true, autostart_on, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出 ClipKeeper", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show_item,
            &settings_item,
            &PredefinedMenuItem::separator(app)?,
            &autostart_item,
            &PredefinedMenuItem::separator(app)?,
            &quit_item,
        ],
    )?;

    // macOS menu bar icons are monochrome templates the system tints for
    // light and dark menu bars; Windows shows the colour app icon.
    #[cfg(target_os = "macos")]
    let (icon, template) = (
        tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png"))?,
        true,
    );
    #[cfg(not(target_os = "macos"))]
    let (icon, template) = (app.default_window_icon().cloned().expect("app icon"), false);

    app.manage(TrayItems {
        show: show_item.clone(),
        autostart: autostart_item.clone(),
    });
    TrayIconBuilder::with_id("main")
        .icon(icon)
        .icon_as_template(template)
        .tooltip("ClipKeeper")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => show(app),
            "settings" => {
                show(app);
                let _ = app.emit(OPEN_SETTINGS, ());
            }
            "autostart" => {
                if let Err(e) = set_launch_at_login(app, !launch_at_login(app)) {
                    eprintln!("{e}");
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}
