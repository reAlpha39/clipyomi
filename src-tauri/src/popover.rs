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
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("popover.html".into()))
        // ta-old's tooltip is `WS_POPUP | WS_BORDER` with
        // `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST`
        // (`MyToolTip.cpp:825`). These are the Tauri equivalents; the border is
        // CSS on the page, since a decorationless window has no frame to style.
        // `WS_EX_NOACTIVATE` is `focusable(false)`, NOT `focused(false)`:
        // `focused` governs creation only, while `show()` below maps to
        // `makeKeyAndOrderFront:` (tauri-runtime-wry `WindowMessage::Show` ->
        // tao `set_visible(true)`), and tao's `canBecomeKeyWindow` returns the
        // `focusable` ivar — which defaults to true. Without this the main
        // window loses its focus ring on every hover.
        .decorations(false)
        .always_on_top(true)
        .focused(false)
        .focusable(false)
        .skip_taskbar(true)
        .visible(false)
        .resizable(false)
        .shadow(false)
        .inner_size(320.0, 120.0)
        .build()?;

    #[cfg(target_os = "windows")]
    {
        if let (Some(main_win), Ok(popover_hwnd)) = (app.get_webview_window("main"), window.hwnd()) {
            if let Ok(main_hwnd) = main_win.hwnd() {
                unsafe {
                    set_window_owner(popover_hwnd.0, main_hwnd.0);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::ffi::c_void;
        if let Ok(ptr) = window.ns_window() {
            if !ptr.is_null() {
                extern "C" {
                    fn objc_msgSend(receiver: *mut c_void, sel: *const c_void, ...) -> *mut c_void;
                    fn sel_registerName(name: *const std::ffi::c_char) -> *const c_void;
                }
                unsafe {
                    let sel_set_level = sel_registerName(c"setLevel:".as_ptr());
                    // NSPopUpMenuWindowLevel is 101, NSToolTipWindowLevel is 102.
                    // Ensures popover stays above NSFloatingWindowLevel (3) when always-on-top is active.
                    let set_level_fn: unsafe extern "C" fn(*mut c_void, *const c_void, isize) =
                        std::mem::transmute(objc_msgSend as *const ());
                    set_level_fn(ptr as *mut c_void, sel_set_level, 101);
                }
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
unsafe fn set_window_owner(child: *mut std::ffi::c_void, owner: *mut std::ffi::c_void) {
    const GWLP_HWNDPARENT: i32 = -8;
    #[cfg(target_pointer_width = "64")]
    extern "system" {
        fn SetWindowLongPtrW(hWnd: *mut std::ffi::c_void, nIndex: i32, dwNewLong: isize) -> isize;
    }
    #[cfg(target_pointer_width = "32")]
    extern "system" {
        fn SetWindowLongW(hWnd: *mut std::ffi::c_void, nIndex: i32, dwNewLong: i32) -> i32;
    }

    #[cfg(target_pointer_width = "64")]
    SetWindowLongPtrW(child, GWLP_HWNDPARENT, owner as isize);
    #[cfg(target_pointer_width = "32")]
    SetWindowLongW(child, GWLP_HWNDPARENT, owner as i32);
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
    window.show().map_err(|e| e.to_string())?;

    #[cfg(target_os = "windows")]
    {
        if let Ok(hwnd) = window.hwnd() {
            unsafe {
                extern "system" {
                    fn SetWindowPos(
                        hWnd: *mut std::ffi::c_void,
                        hWndInsertAfter: *mut std::ffi::c_void,
                        X: i32,
                        Y: i32,
                        cx: i32,
                        cy: i32,
                        uFlags: u32,
                    ) -> i32;
                }
                const HWND_TOPMOST: *mut std::ffi::c_void = -1isize as *mut std::ffi::c_void;
                const SWP_NOSIZE: u32 = 0x0001;
                const SWP_NOMOVE: u32 = 0x0002;
                const SWP_NOACTIVATE: u32 = 0x0010;
                const SWP_SHOWWINDOW: u32 = 0x0040;

                SetWindowPos(
                    hwnd.0,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }
    }

    Ok(())
}

/// Hide it. Not an error when the window is already hidden.
#[tauri::command]
pub fn hide_popover(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or_else(|| format!("no window labelled {LABEL}"))?;
    window.hide().map_err(|e| e.to_string())
}
