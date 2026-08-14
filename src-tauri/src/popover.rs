//! The tooltip window: a second, undecorated, always-on-top webview that can
//! extend past the main window onto the desktop.
//!
//! It exists for the process lifetime and is shown and hidden rather than
//! created per hover — building a webview costs hundreds of milliseconds,
//! which would be plainly visible after the 350 ms dwell that precedes it.

use tauri::{App, Manager, WebviewUrl, WebviewWindowBuilder};

/// The window's label. The capability file and the frontend both name it, so
/// it lives here as one constant rather than three string literals.
pub const LABEL: &str = "popover";

/// Build the hidden tooltip window. Called once from `main`'s `setup`.
pub fn create(app: &App) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("popover.html".into()))
        // ta-old's tooltip is `WS_POPUP | WS_BORDER` with
        // `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST`
        // (`MyToolTip.cpp:825`). These are the Tauri equivalents; the border is
        // CSS on the page, since a decorationless window has no frame to style.
        .decorations(false)
        .always_on_top(true)
        .focused(false)
        .skip_taskbar(true)
        .visible(false)
        .resizable(false)
        .shadow(false)
        .inner_size(320.0, 120.0)
        .build()?;
    Ok(())
}

/// Size, position, and show the tooltip, in that order.
///
/// One command rather than three so the window is never painted at a stale
/// position: it stays hidden until the last statement here.
#[tauri::command]
pub fn place_popover(
    app: tauri::AppHandle,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or_else(|| format!("no window labelled {LABEL}"))?;
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|e| e.to_string())?;
    window
        .set_position(tauri::LogicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())
}

/// Hide it. Not an error when the window is already hidden.
#[tauri::command]
pub fn hide_popover(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or_else(|| format!("no window labelled {LABEL}"))?;
    window.hide().map_err(|e| e.to_string())
}
