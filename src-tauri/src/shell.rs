//! Everything around the history window: showing and hiding it, the global
//! shortcut, the tray icon and its menu, and launch at login.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Emitter, Manager, WebviewWindow};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Default shortcut to open the history window.
pub const SHORTCUT: &str = "CmdOrCtrl+Shift+V";
/// Shown in the tray menu.
#[cfg(target_os = "macos")]
const SHORTCUT_LABEL: &str = "⌘⇧V";
#[cfg(not(target_os = "macos"))]
const SHORTCUT_LABEL: &str = "Ctrl+Shift+V";

/// Argument passed when launched at login, so the app starts in the tray.
pub const HIDDEN_ARG: &str = "--hidden";

/// Event sent to the UI each time the window is shown, so it can reset
/// the search and focus the input.
const WINDOW_SHOWN: &str = "window-shown";

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
    let _ = win.show();
    let _ = win.unminimize();
    let _ = win.set_focus();
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

    register_shortcut(app.handle());
    build_tray(app)?;

    let win = main_window(app.handle()).expect("main window is defined in tauri.conf.json");
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

fn register_shortcut(app: &AppHandle) {
    let shortcut: Shortcut = SHORTCUT.parse().expect("valid shortcut");
    let result = app
        .global_shortcut()
        .on_shortcut(shortcut, |app, _, event| {
            if event.state == ShortcutState::Pressed {
                toggle(app);
            }
        });
    // Another app may own the shortcut; the tray icon still works.
    if let Err(e) = result {
        eprintln!("could not register {SHORTCUT}: {e}");
    }
}

fn build_tray(app: &mut App) -> tauri::Result<()> {
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
    let show_item = MenuItem::with_id(
        app,
        "show",
        format!("显示剪贴板历史    {SHORTCUT_LABEL}"),
        true,
        None::<&str>,
    )?;
    let autostart_item =
        CheckMenuItem::with_id(app, "autostart", "开机自动启动", true, autostart_on, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出 ClipKeeper", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show_item,
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

    let autostart_toggle = autostart_item.clone();
    TrayIconBuilder::with_id("main")
        .icon(icon)
        .icon_as_template(template)
        .tooltip("ClipKeeper")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => show(app),
            "autostart" => {
                let launcher = app.autolaunch();
                let enable = !launcher.is_enabled().unwrap_or(false);
                let result = if enable { launcher.enable() } else { launcher.disable() };
                if let Err(e) = result {
                    eprintln!("failed to change autostart: {e}");
                }
                let _ = autostart_toggle.set_checked(launcher.is_enabled().unwrap_or(false));
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
