//! Background mouse tracking for macOS and Windows.
//!
//! WebKit does not dispatch DOM mousemove/mouseover events when an application
//! is inactive (not key/focused). This module tracks cursor movements across the
//! main window when inactive and emits `unfocused-mouse-move` / `unfocused-mouse-leave`
//! so dictionary tooltips open and close seamlessly without requiring window focus.

use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, serde::Serialize)]
pub struct MouseMovePayload {
    pub x: f64,
    pub y: f64,
    pub screen_x: f64,
    pub screen_y: f64,
}

#[cfg(target_os = "macos")]
pub fn start(app: AppHandle) {
    use std::ffi::c_void;

    std::thread::spawn(move || {
        extern "C" {
            fn objc_msgSend(receiver: *mut c_void, sel: *const c_void, ...) -> *mut c_void;
            fn sel_registerName(name: *const std::ffi::c_char) -> *const c_void;
            fn objc_getClass(name: *const std::ffi::c_char) -> *mut c_void;
        }

        #[repr(C)]
        #[derive(Copy, Clone, Default)]
        struct NSPoint {
            x: f64,
            y: f64,
        }

        let mut was_inside = false;
        let mut last_loc = (0.0f64, 0.0f64);
        let mut win_opt = None;

        loop {
            std::thread::sleep(Duration::from_millis(35));

            if win_opt.is_none() {
                win_opt = app.get_webview_window("main");
            }
            let Some(win) = &win_opt else {
                continue;
            };

            let is_focused = win.is_focused().unwrap_or(false);
            if is_focused {
                if was_inside {
                    was_inside = false;
                    let _ = win.emit("unfocused-mouse-leave", ());
                }
                std::thread::sleep(Duration::from_millis(150));
                continue;
            }

            let Ok(ptr) = win.ns_window() else {
                continue;
            };
            if ptr.is_null() {
                continue;
            }

            unsafe {
                let sel_mouse_location = sel_registerName(c"mouseLocation".as_ptr());
                let ns_event_class = objc_getClass(c"NSEvent".as_ptr());

                if ns_event_class.is_null() {
                    continue;
                }

                #[cfg(target_arch = "aarch64")]
                let mouse_loc: NSPoint = {
                    let mouse_loc_fn: unsafe extern "C" fn(*mut c_void, *const c_void) -> NSPoint =
                        std::mem::transmute(objc_msgSend as *const ());
                    mouse_loc_fn(ns_event_class, sel_mouse_location)
                };

                #[cfg(target_arch = "x86_64")]
                let mouse_loc: NSPoint = {
                    let mouse_loc_fn: unsafe extern "C" fn(*mut NSPoint, *mut c_void, *const c_void) =
                        std::mem::transmute(objc_msgSend as *const ());
                    let mut pt = NSPoint::default();
                    mouse_loc_fn(&mut pt, ns_event_class, sel_mouse_location);
                    pt
                };

                let Ok(inner_pos) = win.inner_position() else { continue; };
                let Ok(inner_size) = win.inner_size() else { continue; };
                let Ok(Some(primary_monitor)) = win.primary_monitor() else { continue; };
                
                let scale_factor = win.scale_factor().unwrap_or(1.0);
                let logical_pos = inner_pos.to_logical::<f64>(scale_factor);
                let logical_size = inner_size.to_logical::<f64>(scale_factor);
                let primary_logical_size = primary_monitor.size().to_logical::<f64>(primary_monitor.scale_factor());
                
                let mouse_logical_x = mouse_loc.x;
                let mouse_logical_y = primary_logical_size.height - mouse_loc.y;

                let is_inside = mouse_logical_x >= logical_pos.x
                    && mouse_logical_x <= (logical_pos.x + logical_size.width)
                    && mouse_logical_y >= logical_pos.y
                    && mouse_logical_y <= (logical_pos.y + logical_size.height);

                if is_inside {
                    if (mouse_logical_x - last_loc.0).abs() > 0.1 || (mouse_logical_y - last_loc.1).abs() > 0.1 {
                        last_loc = (mouse_logical_x, mouse_logical_y);
                        let client_x = mouse_logical_x - logical_pos.x;
                        let client_y = mouse_logical_y - logical_pos.y;
                        let screen_x = mouse_logical_x;
                        let screen_y = mouse_logical_y;

                        let _ = win.emit(
                            "unfocused-mouse-move",
                            MouseMovePayload {
                                x: client_x,
                                y: client_y,
                                screen_x,
                                screen_y,
                            },
                        );
                    }
                    was_inside = true;
                } else if was_inside {
                    was_inside = false;
                    let _ = win.emit("unfocused-mouse-leave", ());
                }
            }
        }
    });
}

#[cfg(target_os = "windows")]
pub fn start(app: AppHandle) {
    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct POINT {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct RECT {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    extern "system" {
        fn GetCursorPos(lpPoint: *mut POINT) -> i32;
        fn GetWindowRect(hWnd: isize, lpRect: *mut RECT) -> i32;
        fn GetClientRect(hWnd: isize, lpRect: *mut RECT) -> i32;
        fn ScreenToClient(hWnd: isize, lpPoint: *mut POINT) -> i32;
        fn GetDpiForWindow(hWnd: isize) -> u32;
    }

    std::thread::spawn(move || {
        let mut was_inside = false;
        let mut last_loc = (0i32, 0i32);
        let mut win_opt = None;

        loop {
            std::thread::sleep(Duration::from_millis(35));

            if win_opt.is_none() {
                win_opt = app.get_webview_window("main");
            }
            let Some(win) = &win_opt else {
                continue;
            };

            let is_focused = win.is_focused().unwrap_or(false);
            if is_focused {
                if was_inside {
                    was_inside = false;
                    let _ = win.emit("unfocused-mouse-leave", ());
                }
                std::thread::sleep(Duration::from_millis(150));
                continue;
            }

            let Ok(hwnd) = win.hwnd() else {
                continue;
            };
            let hwnd_val = hwnd.0;

            unsafe {
                let mut pt = POINT::default();
                if GetCursorPos(&mut pt) == 0 {
                    continue;
                }

                let mut pt_client = POINT { x: pt.x, y: pt.y };
                if ScreenToClient(hwnd_val, &mut pt_client) == 0 {
                    continue;
                }

                let mut client_rect = RECT::default();
                if GetClientRect(hwnd_val, &mut client_rect) == 0 {
                    continue;
                }

                let is_inside = pt_client.x >= client_rect.left && pt_client.x <= client_rect.right && pt_client.y >= client_rect.top && pt_client.y <= client_rect.bottom;

                if is_inside {
                    if (pt.x - last_loc.0).abs() > 0 || (pt.y - last_loc.1).abs() > 0 {
                        last_loc = (pt.x, pt.y);
                        let dpi = GetDpiForWindow(hwnd_val);
                        let scale = if dpi > 0 { dpi as f64 / 96.0 } else { 1.0 };

                        let client_x = pt_client.x as f64 / scale;
                        let client_y = pt_client.y as f64 / scale;
                        let screen_x = pt.x as f64 / scale;
                        let screen_y = pt.y as f64 / scale;

                        let _ = win.emit(
                            "unfocused-mouse-move",
                            MouseMovePayload {
                                x: client_x,
                                y: client_y,
                                screen_x,
                                screen_y,
                            },
                        );
                    }
                    was_inside = true;
                } else if was_inside {
                    was_inside = false;
                    let _ = win.emit("unfocused-mouse-leave", ());
                }
            }
        }
    });
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn start(_app: AppHandle) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_move_payload_serializes_cleanly() {
        let payload = MouseMovePayload {
            x: 120.5,
            y: 45.0,
            screen_x: 320.5,
            screen_y: 245.0,
        };
        let json = serde_json::to_string(&payload).expect("serialization succeeds");
        assert!(json.contains("\"x\":120.5"));
        assert!(json.contains("\"y\":45.0"));
        assert!(json.contains("\"screen_x\":320.5"));
        assert!(json.contains("\"screen_y\":245.0"));
    }

    #[test]
    fn coordinate_containment_and_conversion() {
        let win_origin_x = 100.0f64;
        let win_origin_y = 500.0f64; // Cocoa bottom-left
        let win_width = 400.0f64;
        let win_height = 200.0f64;
        let primary_screen_height = 1080.0f64;

        // Inside window: top-left corner
        let mouse_x = 100.0f64;
        let mouse_y = 700.0f64; // top of window in Cocoa coordinates

        let is_inside = mouse_x >= win_origin_x
            && mouse_x <= (win_origin_x + win_width)
            && mouse_y >= win_origin_y
            && mouse_y <= (win_origin_y + win_height);
        assert!(is_inside);

        let client_x = mouse_x - win_origin_x;
        let client_y = (win_origin_y + win_height) - mouse_y;
        let screen_x = mouse_x;
        let screen_y = primary_screen_height - mouse_y;

        assert_eq!(client_x, 0.0);
        assert_eq!(client_y, 0.0);
        assert_eq!(screen_x, 100.0);
        assert_eq!(screen_y, 380.0);
    }
}
