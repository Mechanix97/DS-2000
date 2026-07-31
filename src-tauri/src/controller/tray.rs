//! Tray menu construction.
//!
//! Here rather than in `main.rs` because changing the language rebuilds the menu, and that
//! happens in a command — which lives in this crate and cannot reach into the binary.

use config::language::Language;
use tauri::menu::{Menu, MenuItem};
use tauri::{AppHandle, Runtime};

/// Identifies the tray icon so it can be found again to be relabelled.
pub const TRAY_ID: &str = "ds2000-tray";

pub const MENU_SHOW: &str = "show";
pub const MENU_QUIT: &str = "quit";

/// Builds the tray menu in the given language.
///
/// Tauri menus are immutable once built, so switching language means building another one and
/// handing it to the tray rather than editing the labels in place.
pub fn tray_menu<R: Runtime>(app: &AppHandle<R>, language: Language) -> tauri::Result<Menu<R>> {
    let show = MenuItem::with_id(app, MENU_SHOW, language.tray_show(), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, language.tray_quit(), true, None::<&str>)?;
    Menu::with_items(app, &[&show, &quit])
}
