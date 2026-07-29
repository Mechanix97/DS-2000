#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use controller::commands;
use controller::controller::Controller;
use controller::coordinator::{Coordinator, UiRefreshHandle};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info};

/// Window the frontend runs in.
const MAIN_WINDOW: &str = "main";

/// Signals that shutdown has finished and the process may exit.
///
/// A newtype rather than a bare `UnboundedSender<()>`: Tauri keys managed state by type, and
/// `UiRefreshHandle` is also an `UnboundedSender<()>`, so registering both as-is panics at
/// startup with "state for type ... is already being managed".
#[derive(Clone)]
struct ShutdownSignal(mpsc::UnboundedSender<()>);

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("App started");

    let (controller, worker_events) = Controller::new().await;
    let controller = Arc::new(Mutex::new(controller));

    if let Err(err) = controller.lock().await.start().await {
        error!("Controller could not start: {err}");
        std::process::exit(1);
    }

    let (ui_refresh_tx, ui_refresh_rx) = mpsc::unbounded_channel();
    // Signals that shutdown finished, replacing a thread that used to poll a flag ten times a
    // second for the whole life of the process.
    let (shutdown_tx, mut shutdown_rx) = mpsc::unbounded_channel::<()>();

    tauri::Builder::default()
        // TODO: add a single-instance guard. Without one, a second launch fights the first for
        // the serial port and the Discord pipe. The `tauri-plugin-single-instance` dependency
        // was dropped while the feature stays unimplemented; re-add it when picking this up.
        .plugin(tauri_plugin_shell::init())
        .manage(controller.clone())
        .manage(ui_refresh_tx.clone())
        .manage(ShutdownSignal(shutdown_tx))
        .invoke_handler(tauri::generate_handler![
            commands::controller_start,
            commands::ds_set_voice_settings_command,
            commands::serial_set_rgb,
            commands::discord_credentials_status,
            commands::discord_set_credentials,
            commands::discord_clear_credentials,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                debug!("Closing the window hides the app instead of quitting");
                api.prevent_close();
                if let Err(err) = window.hide() {
                    debug!("Could not hide the window: {err}");
                }
            }
        })
        .setup(move |app| {
            let quit_item = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
            let show_item = MenuItem::with_id(app, "show", "Abrir DS2000", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let mut tray = TrayIconBuilder::new().menu(&menu);
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            } else {
                // Not fatal: the tray still works, it just has no icon.
                debug!("No default window icon available for the tray");
            }

            tray.on_menu_event(|app_handle, event| match event.id.as_ref() {
                "quit" => quit(app_handle.clone()),
                "show" => show_window(app_handle),
                other => debug!("Unhandled tray menu item: {other}"),
            })
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    show_window(tray.app_handle());
                }
            })
            .build(app)?;

            // The coordinator reacts to the workers; it needs the app handle to reach the webview.
            let coordinator = Coordinator::new(app.handle().clone(), controller.clone());
            tauri::async_runtime::spawn(coordinator.run(
                worker_events.discord,
                worker_events.serial,
                ui_refresh_rx,
            ));

            // Exits once shutdown reports it is done, instead of polling for it.
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if shutdown_rx.recv().await.is_some() {
                    info!("Shutdown completed, exiting");
                    app_handle.exit(0);
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Brings the window back and asks for a fresh state.
///
/// The refresh matters because nothing is emitted while the window is hidden, so without it the
/// UI would show whatever it last saw before being put away.
fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    if let Some(refresh) = app.try_state::<UiRefreshHandle>() {
        let _ = refresh.send(());
    }
}

/// Shuts the workers down and then signals the exit task.
fn quit(app: AppHandle) {
    let controller = app.state::<Arc<Mutex<Controller>>>().inner().clone();
    let shutdown = app.state::<ShutdownSignal>().inner().clone();

    tauri::async_runtime::spawn(async move {
        if let Err(err) = controller.lock().await.shutdown().await {
            // Report it and exit anyway: refusing to quit is worse than an unclean shutdown.
            error!("Error shutting down the controller: {err}");
        }
        let _ = shutdown.0.send(());
    });
}
