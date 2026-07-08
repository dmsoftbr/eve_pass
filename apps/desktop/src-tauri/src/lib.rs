//! EVEPass desktop backend. Holds the vault `Session` and encrypted cache in
//! Rust; the React frontend does Supabase I/O and never sees key material.
//!
//! Fase 2 adds: tray/menu bar with lock state, run-in-background (hide on close),
//! a frameless command-palette window toggled by a global hotkey, inactivity
//! auto-lock, and settings (hotkey/autostart re-applied live).

mod cache;
mod commands;
mod settings;
mod state;

use std::time::Duration;

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::{ManagerExt, MacosLauncher};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use settings::Settings;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .setup(|app| {
            let dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("evepass");
            let settings = Settings::load(&dir);
            app.manage(AppState::new(dir, settings.clone()));

            // Run as a menu-bar app: no Dock icon, accessible via the tray.
            #[cfg(target_os = "macos")]
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            build_tray(app.handle())?;
            hide_on_close(app.handle());
            apply_settings(app.handle(), &settings);
            spawn_auto_lock(app.handle());
            update_tray(app.handle(), false);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::vault_status,
            commands::create_account,
            commands::begin_login,
            commands::complete_login,
            commands::lock,
            commands::list_items,
            commands::get_item,
            commands::save_item,
            commands::delete_item,
            commands::mark_synced,
            commands::copy_field,
            commands::list_folders,
            commands::save_folder,
            commands::delete_folder,
            commands::apply_remote_changes,
            commands::pending_uploads,
            commands::gen_password,
            // Fase 2
            commands::vault_health,
            commands::breach_prefixes,
            commands::resolve_breaches,
            commands::item_totp,
            commands::palette_search,
            commands::save_items_batch,
            commands::save_folders_batch,
            commands::get_settings,
            commands::set_settings,
            commands::ping_activity,
            // Fase 4 (collections + recovery)
            commands::create_collection,
            commands::load_collection_keys,
            commands::wrap_collection_key_for,
            commands::decrypt_collection_name,
            commands::rotate_collection_key,
            commands::public_key_fingerprint,
            commands::reset_password,
            commands::unlock_with_recovery,
            commands::delete_collection_cache,
        ])
        .run(tauri::generate_context!())
        .expect("error while running EVEPass");
}

/// Build the menu-bar/tray icon and its menu.
fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Abrir EVEPass", true, None::<&str>)?;
    let palette = MenuItem::with_id(app, "palette", "Command palette", true, None::<&str>)?;
    let lock = MenuItem::with_id(app, "lock", "Travar", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &palette, &lock, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .icon_as_template(true) // monochrome, macOS tints it for light/dark bars
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "palette" => toggle_palette(app),
            "lock" => {
                let state = app.state::<AppState>();
                commands::perform_lock(app, &state);
                let _ = app.emit("vault-locked", ());
            }
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = tray_icon(false) {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

/// The monochrome menu-bar lock icon (closed when locked, open when unlocked).
fn tray_icon(unlocked: bool) -> Option<Image<'static>> {
    let bytes: &[u8] = if unlocked {
        include_bytes!("../icons/tray-unlocked.png")
    } else {
        include_bytes!("../icons/tray-locked.png")
    };
    Image::from_bytes(bytes).ok()
}

/// Show the Dock icon only while the main window is visible (Regular), and hide
/// it (Accessory / menu-bar-only) when the window is closed to the tray.
fn set_dock_visible(app: &AppHandle, visible: bool) {
    #[cfg(target_os = "macos")]
    {
        let policy = if visible {
            tauri::ActivationPolicy::Regular
        } else {
            tauri::ActivationPolicy::Accessory
        };
        let _ = app.set_activation_policy(policy);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, visible);
}

/// Closing the main window hides it to the tray instead of quitting.
fn hide_on_close(app: &AppHandle) {
    let handle = app.clone();
    if let Some(win) = app.get_webview_window("main") {
        win.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Some(w) = handle.get_webview_window("main") {
                    let _ = w.hide();
                }
                set_dock_visible(&handle, false); // window hidden → no Dock icon
            }
        });
    }
}

fn show_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        set_dock_visible(app, true); // window visible → show in Dock
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// Show/hide the frameless command-palette window.
fn toggle_palette(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("palette") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            let _ = win.center();
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

/// Re-register the global hotkey and autostart from current settings.
pub fn apply_settings(app: &AppHandle, s: &Settings) {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let _ = gs.on_shortcut(s.global_hotkey.as_str(), |app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            toggle_palette(app);
        }
    });

    let autostart = app.autolaunch();
    if s.launch_at_login {
        let _ = autostart.enable();
    } else {
        let _ = autostart.disable();
    }
}

/// Reflect lock state by swapping the single menu-bar lock icon (open/closed).
pub fn update_tray(app: &AppHandle, unlocked: bool) {
    if let Some(tray) = app.tray_by_id("main") {
        if let Some(img) = tray_icon(unlocked) {
            let _ = tray.set_icon(Some(img));
            let _ = tray.set_icon_as_template(true);
        }
    }
}

/// Background thread that locks the vault after `auto_lock_minutes` of inactivity.
fn spawn_auto_lock(app: &AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(10));
        let state = handle.state::<AppState>();
        let should_lock = {
            let inner = state.inner.lock().unwrap();
            match (inner.session.is_some(), inner.settings.auto_lock_minutes, inner.last_activity) {
                (true, mins, Some(last)) if mins > 0 => {
                    last.elapsed() >= Duration::from_secs(mins as u64 * 60)
                }
                _ => false,
            }
        };
        if should_lock {
            commands::perform_lock(&handle, &state);
            let _ = handle.emit("vault-locked", ());
        }
    });
}
