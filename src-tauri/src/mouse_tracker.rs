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
        #[derive(Copy, Clone, Default, Debug, PartialEq)]
        struct NSPoint {
            x: f64,
            y: f64,
        }

        #[repr(C)]
        #[derive(Copy, Clone, Default, Debug, PartialEq)]
        struct NSRect {
            x: f64,
            y: f64,
            width: f64,
            height: f64,
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
                let sel_frame = sel_registerName(c"frame".as_ptr());
                let sel_style_mask = sel_registerName(c"styleMask".as_ptr());
                let sel_title_vis = sel_registerName(c"titleVisibility".as_ptr());
                let sel_is_trans = sel_registerName(c"titlebarAppearsTransparent".as_ptr());
                let sel_frame_rect_for_content = sel_registerName(c"frameRectForContentRect:styleMask:".as_ptr());
                let sel_screens = sel_registerName(c"screens".as_ptr());
                let sel_first_object = sel_registerName(c"firstObject".as_ptr());

                let ns_window_class = objc_getClass(c"NSWindow".as_ptr());
                let ns_event_class = objc_getClass(c"NSEvent".as_ptr());
                let ns_screen_class = objc_getClass(c"NSScreen".as_ptr());

                if ns_window_class.is_null() || ns_event_class.is_null() || ns_screen_class.is_null() {
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

                #[cfg(target_arch = "aarch64")]
                let win_frame: NSRect = {
                    let frame_fn: unsafe extern "C" fn(*mut c_void, *const c_void) -> NSRect =
                        std::mem::transmute(objc_msgSend as *const ());
                    frame_fn(ptr, sel_frame)
                };

                #[cfg(target_arch = "x86_64")]
                let win_frame: NSRect = {
                    let frame_fn: unsafe extern "C" fn(*mut NSRect, *mut c_void, *const c_void) =
                        std::mem::transmute(objc_msgSend as *const ());
                    let mut r = NSRect::default();
                    frame_fn(&mut r, ptr, sel_frame);
                    r
                };

                let style_mask_fn: unsafe extern "C" fn(*mut c_void, *const c_void) -> usize =
                    std::mem::transmute(objc_msgSend as *const ());
                let style_mask = style_mask_fn(ptr, sel_style_mask);

                let title_vis_fn: unsafe extern "C" fn(*mut c_void, *const c_void) -> isize =
                    std::mem::transmute(objc_msgSend as *const ());
                let title_visibility = title_vis_fn(ptr, sel_title_vis);

                let is_trans_fn: unsafe extern "C" fn(*mut c_void, *const c_void) -> bool =
                    std::mem::transmute(objc_msgSend as *const ());
                let is_transparent = is_trans_fn(ptr, sel_is_trans);

                let title_bar_height = if (style_mask & 1) != 0 && !(title_visibility == 1 && is_transparent) {
                    #[cfg(target_arch = "aarch64")]
                    {
                        let frame_rect_fn: unsafe extern "C" fn(*mut c_void, *const c_void, NSRect, usize) -> NSRect =
                            std::mem::transmute(objc_msgSend as *const ());
                        let sample = frame_rect_fn(ns_window_class, sel_frame_rect_for_content, NSRect { x: 0.0, y: 0.0, width: 100.0, height: 100.0 }, 15);
                        sample.height - 100.0
                    }
                    #[cfg(target_arch = "x86_64")]
                    {
                        let frame_rect_fn: unsafe extern "C" fn(*mut NSRect, *mut c_void, *const c_void, NSRect, usize) =
                            std::mem::transmute(objc_msgSend as *const ());
                        let mut sample = NSRect::default();
                        frame_rect_fn(&mut sample, ns_window_class, sel_frame_rect_for_content, NSRect { x: 0.0, y: 0.0, width: 100.0, height: 100.0 }, 15);
                        sample.height - 100.0
                    }
                } else {
                    0.0
                };

                let screens_fn: unsafe extern "C" fn(*mut c_void, *const c_void) -> *mut c_void =
                    std::mem::transmute(objc_msgSend as *const ());
                let screens = screens_fn(ns_screen_class, sel_screens);
                let first_screen = if !screens.is_null() {
                    let first_fn: unsafe extern "C" fn(*mut c_void, *const c_void) -> *mut c_void =
                        std::mem::transmute(objc_msgSend as *const ());
                    first_fn(screens, sel_first_object)
                } else {
                    std::ptr::null_mut()
                };

                let primary_screen_height = if !first_screen.is_null() {
                    #[cfg(target_arch = "aarch64")]
                    {
                        let frame_fn: unsafe extern "C" fn(*mut c_void, *const c_void) -> NSRect =
                            std::mem::transmute(objc_msgSend as *const ());
                        frame_fn(first_screen, sel_frame).height
                    }
                    #[cfg(target_arch = "x86_64")]
                    {
                        let frame_fn: unsafe extern "C" fn(*mut NSRect, *mut c_void, *const c_void) =
                            std::mem::transmute(objc_msgSend as *const ());
                        let mut r = NSRect::default();
                        frame_fn(&mut r, first_screen, sel_frame);
                        r.height
                    }
                } else {
                    1080.0
                };

                let content_left = win_frame.x;
                let content_bottom = win_frame.y;
                let content_width = win_frame.width;
                let content_height = win_frame.height - title_bar_height;
                let content_top_cocoa = content_bottom + content_height;

                let is_inside = mouse_loc.x >= content_left
                    && mouse_loc.x <= (content_left + content_width)
                    && mouse_loc.y >= content_bottom
                    && mouse_loc.y <= content_top_cocoa;

                if is_inside {
                    if (mouse_loc.x - last_loc.0).abs() > 0.1 || (mouse_loc.y - last_loc.1).abs() > 0.1 {
                        last_loc = (mouse_loc.x, mouse_loc.y);
                        let client_x = mouse_loc.x - content_left;
                        let client_y = content_top_cocoa - mouse_loc.y;
                        let screen_x = mouse_loc.x;
                        let screen_y = primary_screen_height - mouse_loc.y;

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

    #[test]
    fn titled_window_coordinate_conversion() {
        let win_frame_x = 100.0f64;
        let win_frame_y = 500.0f64;
        let win_frame_width = 720.0f64;
        let win_frame_height = 512.0f64;
        let title_bar_height = 32.0f64;
        let primary_screen_height = 1080.0f64;

        let content_left = win_frame_x;
        let content_bottom = win_frame_y;
        let content_width = win_frame_width;
        let content_height = win_frame_height - title_bar_height;
        let content_top_cocoa = content_bottom + content_height;

        // Inside content view (top-left of webview content view)
        let mouse_x = 100.0f64;
        let mouse_y = 980.0f64; // content_top_cocoa (500 + 480)

        let is_inside = mouse_x >= content_left
            && mouse_x <= (content_left + content_width)
            && mouse_y >= content_bottom
            && mouse_y <= content_top_cocoa;
        assert!(is_inside);

        let client_x = mouse_x - content_left;
        let client_y = content_top_cocoa - mouse_y;
        let screen_x = mouse_x;
        let screen_y = primary_screen_height - mouse_y;

        assert_eq!(client_x, 0.0);
        assert_eq!(client_y, 0.0);
        assert_eq!(screen_x, 100.0);
        assert_eq!(screen_y, 100.0);

        // Point inside title bar (y = 1000 > content_top_cocoa = 980)
        let mouse_in_title_y = 1000.0f64;
        let is_title_inside = mouse_x >= content_left
            && mouse_x <= (content_left + content_width)
            && mouse_in_title_y >= content_bottom
            && mouse_in_title_y <= content_top_cocoa;
        assert!(!is_title_inside);
    }
}

