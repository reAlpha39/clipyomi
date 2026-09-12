// SPDX-License-Identifier: MIT OR Apache-2.0

//! The webview's entry points: push input, toggle settings, read settings, ask
//! why startup failed or why settings fell back to defaults, and signal that
//! the event listeners are registered. The dictionary download, the kanji
//! lookup and the hints toggle live in [`dictionary`] and [`hints`]. Parsing
//! itself runs in `parse::run_worker`; results arrive as `parse-result` /
//! `parse-error` events, not as a command return value.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{Manager, State, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::{watch, Notify};

use crate::settings::{ChromeMode, Settings};
use crate::state::{SettingsState, SettingsWarning, ShortcutWarning, StartupFailure};

pub mod dictionary;
pub mod fonts;
pub mod hints;

/// The sending half of the input channel, managed so commands can reach it.
pub struct InputSender(pub watch::Sender<String>);

/// Signals that the webview has registered its `parse-result` /
/// `parse-error` listeners. `frontend_ready` fires it; `clipboard::run_poll`
/// blocks its first tick on it — see `clipboard::wait_for_frontend` for why.
pub struct FrontendReady(pub Arc<Notify>);

/// Publish text for the worker to parse.
///
/// Separated from the command so it can be tested without a Tauri app handle.
fn push_input(tx: &watch::Sender<String>, text: String) -> Result<(), String> {
    tx.send(text)
        .map_err(|_| "the parse worker is no longer running".to_string())
}

/// Queue TEXT for parsing. The result arrives as a `parse-result` event.
///
/// Fire-and-forget rather than request/response: the clipboard produces parses
/// nobody asked for, so the webview renders from the event stream either way and
/// a returned value here would be a second, redundant path.
///
/// **This command deliberately has no caller.** Phase 2H removed the manual
/// text box, making the clipboard the only user-facing input path, and with it
/// the one `invoke` that reached this. It is kept as a test and debug entry
/// point: with clipboard-only input, exercising a parse by hand otherwise means
/// putting text on the system clipboard for every attempt, and this can be
/// driven from the DevTools console instead. Not dead code — do not remove it
/// without replacing that affordance.
#[tauri::command]
pub fn set_input(text: String, input: State<'_, InputSender>) -> Result<(), String> {
    push_input(&input.0, text)
}

#[tauri::command]
pub fn set_clipboard_monitoring(
    enabled: bool,
    settings: State<'_, Arc<SettingsState>>,
) -> Result<(), String> {
    settings
        .update(|s| s.clipboard_monitoring = enabled)
        .map_err(|e| e.to_string())
}

/// The window the chrome controls act on, resolved by label.
///
/// Deliberately *not* the `tauri::Window` Tauri injects into a command, which
/// is whichever webview invoked it. The settings window invokes these commands
/// too, so an injected caller made the settings window pin or undecorate
/// itself instead of the main window. `main.rs`'s startup restore already
/// resolves `"main"` this way.
pub(crate) fn chrome_target<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<tauri::Window<R>, String> {
    app.get_webview_window("main")
        .map(|w| w.as_ref().window())
        .ok_or_else(|| "main window not found".to_string())
}

#[tauri::command]
pub fn set_always_on_top(
    enabled: bool,
    app: tauri::AppHandle,
    settings: State<'_, Arc<SettingsState>>,
) -> Result<(), String> {
    chrome_target(&app)?
        .set_always_on_top(enabled)
        .map_err(|e| e.to_string())?;
    settings
        .update(|s| s.always_on_top = enabled)
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
/// Shows or hides the two visible halves of the title bar: the title text and
/// the standard window buttons.
///
/// Deliberately narrow. It does NOT touch `setStyleMask:` or
/// `setTitlebarAppearsTransparent:` — those decide *who owns the titlebar
/// strip*, and the window is created `titleBarStyle: "Overlay"` so the answer
/// is always "the webview". Flipping transparency here is what caused the blink
/// loop: the OS took the strip mid-hover, the webview stopped receiving pointer
/// events in it, `pointerleave` fired, the frontend undid the peek, the strip
/// came back, `pointerenter` fired, and round it went at pointer rate (measured
/// at five full cycles from a single hover).
fn set_titlebar_chrome(window: &tauri::Window, visible: bool) -> Result<(), String> {
    use std::ffi::c_void;
    let ptr_addr = window.ns_window().map_err(|e| e.to_string())? as usize;
    window
        .run_on_main_thread(move || {
            let ptr = ptr_addr as *mut c_void;
            extern "C" {
                fn objc_msgSend(receiver: *mut c_void, sel: *const c_void, ...) -> *mut c_void;
                fn sel_registerName(name: *const std::ffi::c_char) -> *const c_void;
            }
            unsafe {
                let sel_set_title_visibility = sel_registerName(c"setTitleVisibility:".as_ptr());
                let sel_standard_window_button =
                    sel_registerName(c"standardWindowButton:".as_ptr());
                let sel_set_hidden = sel_registerName(c"setHidden:".as_ptr());

                // 0 = Visible, 1 = Hidden
                let title_visibility: isize = if visible { 0 } else { 1 };
                let set_title_vis_fn: unsafe extern "C" fn(*mut c_void, *const c_void, isize) =
                    std::mem::transmute(objc_msgSend as *const ());
                set_title_vis_fn(ptr, sel_set_title_visibility, title_visibility);

                // 0 = Close, 1 = Miniaturize, 2 = Zoom, 7 = FullScreen
                let get_btn_fn: unsafe extern "C" fn(
                    *mut c_void,
                    *const c_void,
                    usize,
                ) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ());
                let set_hidden_fn: unsafe extern "C" fn(*mut c_void, *const c_void, bool) =
                    std::mem::transmute(objc_msgSend as *const ());

                for button_type in [0usize, 1, 2, 7] {
                    let btn = get_btn_fn(ptr, sel_standard_window_button, button_type);
                    if !btn.is_null() {
                        set_hidden_fn(btn, sel_set_hidden, !visible);
                    }
                }
            }
        })
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
pub fn apply_decorations_macos(window: &tauri::Window, enabled: bool) -> Result<(), String> {
    set_titlebar_chrome(window, enabled)?;
    // Kept for the *setting* only (9e6d01b: a decorations toggle used to leave
    // the webview on a stale theme). A repaint is invisible against a
    // deliberate toggle, and `peek_titlebar` skips it — repainting on every
    // hover crossing is the flash half of the blink.
    let win = window.clone();
    window
        .run_on_main_thread(move || {
            let _ = win.set_theme(win.theme().ok());
        })
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
// `RECT`, `STYLESTRUCT`, `WINDOWPOS` and `SUBCLASSPROC` are Win32's own names
// and are kept spelled the way MSDN spells them: every field layout and every
// `extern "system"` signature in here is read against that documentation, and
// a renamed type makes the comparison one step harder for no gain. Scoped to
// this module so the lint still applies everywhere else.
//
// It is an `allow` rather than a rename because of an asymmetry that would
// otherwise go unnoticed: this module is `cfg(target_os = "windows")`, so
// `.github/workflows/ci.yml`'s `clippy --workspace --all-targets -- -D
// warnings` never compiles it on `ubuntu-latest` and never reports these —
// while the same command on the machine this is developed on fails with four
// errors. The declared standard has to be runnable where the code is written.
#[allow(clippy::upper_case_acronyms)]
pub mod win32_frameless {
    use tauri::Window;

    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    static SUBCLASSED: AtomicBool = AtomicBool::new(false);

    /// Whether the band's reserved row is currently inside the region.
    ///
    /// The window is permanently `BAND_HEIGHT_LOGICAL` taller than it looks and
    /// sits that much higher; at rest the region clips that row away, so the
    /// desktop shows through it and the window appears to start at the content.
    /// A peek puts the row back into the region, which is the whole reveal —
    /// nothing resizes, nothing moves, and the row's pixels were painted by the
    /// webview all along.
    ///
    /// Measured against the alternative of growing the frame: 39% of reveals
    /// flashed the desktop for a frame that way, 1% this way. The difference is
    /// the page-wide reflow that `peek-offset` used to trigger, which this
    /// design removes by reserving the row permanently.
    static STRIP_SHOWN: AtomicBool = AtomicBool::new(false);

    /// The window's background, as a COLORREF, kept in step with the theme the
    /// webview renders. `WM_ERASEBKGND` paints it.
    static BACKGROUND: AtomicU32 = AtomicU32::new(BACKGROUND_DARK);

    /// Whether the page's background is fully opaque. Starts `true`: the
    /// window is opaque until the renderer says otherwise, which is the same
    /// direction `BACKGROUND`'s theme guess errs in.
    static BACKGROUND_OPAQUE: AtomicBool = AtomicBool::new(true);

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct RECT {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct NCCALCSIZE_PARAMS {
        rgrc: [RECT; 3],
        lppos: *mut c_void,
    }

    #[repr(C)]
    struct STYLESTRUCT {
        style_old: u32,
        style_new: u32,
    }

    /// `WVR_HREDRAW | WVR_VREDRAW`: redraw the whole client area on resize
    /// rather than blitting the old one. The fallback for a move that keeps no
    /// pixels at all — see `WVR_VALIDRECTS`.
    const WVR_REDRAW: isize = 0x0300;

    /// `rgrc[1]` and `rgrc[2]` carry the destination and the source of the blit
    /// Windows performs across the resize, instead of it choosing.
    const WVR_VALIDRECTS: isize = 0x0400;

    const WM_STYLECHANGING: u32 = 0x007C;
    const WM_NCCALCSIZE: u32 = 0x0083;
    const WM_NCHITTEST: u32 = 0x0084;
    const WM_NCPAINT: u32 = 0x0085;
    const WM_NCACTIVATE: u32 = 0x0086;
    const WM_ERASEBKGND: u32 = 0x0014;
    const WM_WINDOWPOSCHANGING: u32 = 0x0046;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct WINDOWPOS {
        hwnd: *mut c_void,
        hwndInsertAfter: *mut c_void,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    }
    const WM_DPICHANGED: u32 = 0x02E0;

    /// `--color-bg` from `tokens.css`, as COLORREF (0x00BBGGRR). Both themes
    /// are neutral greys, so the byte order reorders nothing.
    const BACKGROUND_DARK: u32 = 0x0012_1212;
    const BACKGROUND_LIGHT: u32 = 0x00FC_FCFC;

    /// The corner radius, in logical pixels, of the window region. Matches the
    /// radius Win11 itself rounds a window to.
    pub const CORNER_RADIUS_LOGICAL: i32 = 8;

    const HTCLIENT: isize = 1;
    const HTCAPTION: isize = 2;
    const HTLEFT: isize = 10;
    const HTRIGHT: isize = 11;
    const HTTOP: isize = 12;
    const HTTOPLEFT: isize = 13;
    const HTTOPRIGHT: isize = 14;
    const HTBOTTOM: isize = 15;
    const HTBOTTOMLEFT: isize = 16;
    const HTBOTTOMRIGHT: isize = 17;

    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_DONOTROUND: u32 = 1;
    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    const DWMWA_BORDER_COLOR: u32 = 34;
    const DWMWA_CAPTION_COLOR: u32 = 35;
    const DWMWA_COLOR_NONE: u32 = 0xFFFFFFFE;

    const SM_CXSIZEFRAME: i32 = 32;
    const SM_CYSIZEFRAME: i32 = 33;
    const SM_CXPADDEDBORDER: i32 = 92;

    const GWL_STYLE: i32 = -16;
    const WS_THICKFRAME: u32 = 0x00040000;
    const WS_CAPTION: u32 = 0x00C00000;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_FRAMECHANGED: u32 = 0x0020;

    #[allow(non_snake_case)]
    type SUBCLASSPROC = unsafe extern "system" fn(
        hWnd: *mut c_void,
        uMsg: u32,
        wParam: usize,
        lParam: isize,
        uIdSubclass: usize,
        dwRefData: usize,
    ) -> isize;

    #[allow(non_snake_case)]
    extern "system" {
        fn GetWindowRect(hWnd: *mut c_void, lpRect: *mut RECT) -> i32;
        fn CreateRoundRectRgn(x1: i32, y1: i32, x2: i32, y2: i32, w: i32, h: i32) -> *mut c_void;
        fn CreateRectRgn(x1: i32, y1: i32, x2: i32, y2: i32) -> *mut c_void;
        fn SetWindowRgn(hWnd: *mut c_void, hRgn: *mut c_void, bRedraw: i32) -> i32;
        fn GetClientRect(hWnd: *mut c_void, lpRect: *mut RECT) -> i32;
        fn CreateSolidBrush(color: u32) -> *mut c_void;
        fn FillRect(hDC: *mut c_void, lprc: *const RECT, hbr: *mut c_void) -> i32;
        fn DeleteObject(ho: *mut c_void) -> i32;
        fn GetDpiForWindow(hWnd: *mut c_void) -> u32;
        fn GetSystemMetricsForDpi(nIndex: i32, dpi: u32) -> i32;
        fn IsZoomed(hWnd: *mut c_void) -> i32;
        fn GetWindowLongPtrW(hWnd: *mut c_void, nIndex: i32) -> isize;
        fn SetWindowLongPtrW(hWnd: *mut c_void, nIndex: i32, dwNewLong: isize) -> isize;
        fn SetWindowPos(
            hWnd: *mut c_void,
            hWndInsertAfter: *mut c_void,
            X: i32,
            Y: i32,
            cx: i32,
            cy: i32,
            uFlags: u32,
        ) -> i32;
        fn DwmSetWindowAttribute(
            hWnd: *mut c_void,
            dwAttribute: u32,
            pvAttribute: *const c_void,
            cbAttribute: u32,
        ) -> i32;
        fn SetWindowSubclass(
            hWnd: *mut c_void,
            pfnSubclass: SUBCLASSPROC,
            uIdSubclass: usize,
            dwRefData: usize,
        ) -> i32;
        fn DefSubclassProc(hWnd: *mut c_void, uMsg: u32, wParam: usize, lParam: isize) -> isize;
    }

    fn mul_div(n_number: i32, n_numerator: i32, n_denominator: i32) -> i32 {
        if n_denominator == 0 {
            return 0;
        }
        ((n_number as i64 * n_numerator as i64) / n_denominator as i64) as i32
    }

    /// The reserved row's height in physical pixels.
    fn strip_px(dpi: i32) -> i32 {
        mul_div(super::BAND_HEIGHT_LOGICAL as i32, dpi, 96)
    }

    /// How much of the window's top the region clips away.
    ///
    /// Zero while the band is peeked, `strip_px` otherwise. There is no
    /// decorated case to check any more: `native` does not exist on Windows
    /// (spec §4), so the row is always reserved and always clipped at rest.
    /// This used to read `WS_CAPTION` off the live window to answer that
    /// question, which could only ever answer "undecorated" — the
    /// `WM_STYLECHANGING` arm below strips the bit for the window's life.
    fn region_top_inset(dpi: i32) -> i32 {
        if STRIP_SHOWN.load(Ordering::Relaxed) {
            0
        } else {
            strip_px(dpi)
        }
    }

    /// Where the window *looks* like it starts: its own top, plus whatever the
    /// region clips off it.
    fn visual_top(window_top: i32, dpi: i32) -> i32 {
        window_top + region_top_inset(dpi)
    }

    fn get_system_metrics(index: i32, dpi: u32) -> i32 {
        unsafe { GetSystemMetricsForDpi(index, dpi) }
    }

    /// The pixels that survive a move-or-resize: what the old and the new
    /// client areas have in common, in screen coordinates.
    ///
    /// Handed to Windows as BOTH the source and the destination of its blit, so
    /// the survivors keep their screen position and the only invalid area is
    /// what the resize newly exposed. The default (a zero return) anchors the
    /// blit at the client top-left instead, which shifts every pixel on screen
    /// when the top edge moves and leaves the unfilled strip at the opposite
    /// edge.
    ///
    /// May be empty — a window can be moved further than its own size in one
    /// step. The caller falls back to `WVR_REDRAW` then.
    fn valid_rect(old_client: RECT, new_client: RECT) -> RECT {
        RECT {
            left: old_client.left.max(new_client.left),
            top: old_client.top.max(new_client.top),
            right: old_client.right.min(new_client.right),
            bottom: old_client.bottom.min(new_client.bottom),
        }
    }

    /// The style bits a frameless main window must carry.
    ///
    /// `WS_THICKFRAME` stays: `DefWindowProc` refuses to resize a window
    /// without it, whatever `WM_NCHITTEST` answers, and dropping it was
    /// measured not to remove the shadow. `WS_CAPTION` goes — `WM_NCCALCSIZE`
    /// only hides that frame, it does not unregister it.
    fn frameless_style(style: u32) -> u32 {
        (style | WS_THICKFRAME) & !WS_CAPTION
    }

    /// Rounds the window by clipping it to a round-rect region.
    ///
    /// `DWMWCP_ROUND` would be the one-line version, but Win11 draws that
    /// rounded visual together with a drop shadow that no style bit or
    /// non-client rendering policy turns off. A region has no shadow. The cost
    /// is a hard-clipped, unantialiased corner, and having to re-apply the
    /// region on every resize — the `WM_WINDOWPOSCHANGING` arm below does that.
    ///
    /// `width` and `height` are the size the window is *about to* have, taken
    /// from the incoming `WINDOWPOS`. Reading the size back off the window
    /// instead left the region a step behind a top-edge drag — that drag moves
    /// the origin and grows the window in one operation, and a region sized
    /// for the previous frame clipped a see-through strip off the bottom.
    ///
    /// A maximized window keeps a rectangular region rather than none: its
    /// corners meet the work area so rounding them would show the desktop
    /// through the gap, but the reserved row still has to be clipped away, and
    /// dropping the region entirely would expose it.
    ///
    /// The region's top edge is the reserved row's height whenever that row is
    /// hidden, which is what makes the window look like it starts at its
    /// content while really extending above it.
    fn apply_round_region(hwnd: *mut c_void, width: i32, height: i32) {
        unsafe {
            let dpi = GetDpiForWindow(hwnd);
            let dpi_val = if dpi == 0 { 96 } else { dpi as i32 };
            let top = region_top_inset(dpi_val);

            if IsZoomed(hwnd) != 0 {
                let rgn = CreateRectRgn(0, top, width, height);
                if !rgn.is_null() {
                    SetWindowRgn(hwnd, rgn, 0);
                }
                return;
            }

            let diameter = 2 * mul_div(CORNER_RADIUS_LOGICAL, dpi_val, 96);

            // `CreateRoundRectRgn` treats the lower-right corner as exclusive,
            // so the rect runs one past the window's width and height.
            let rgn = CreateRoundRectRgn(0, top, width + 1, height + 1, diameter, diameter);
            if !rgn.is_null() {
                // The window owns the region from here — deleting it would
                // free memory the system is still clipping against.
                //
                // `bRedraw: 0`: a redraw here invalidates the WHOLE window,
                // which is the one thing the `WVR_VALIDRECTS` return above
                // exists to avoid — every resize would repaint the client and
                // flash the desktop through it. The region still clips from
                // this call on, and the corners it changes fall inside what the
                // resize repaints anyway.
                SetWindowRgn(hwnd, rgn, 0);
            }
        }
    }

    pub fn apply_dwm_attributes(hwnd: *mut c_void) {
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, frameless_style(style) as isize);
            let _ = SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );

            // Win11 draws the rounded-window visual together with its own drop
            // shadow, and that shadow outlives the loss of every frame style
            // bit. Opting out of the rounding is what removes it.
            let corner_pref: u32 = DWMWCP_DONOTROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner_pref as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            // Rounding is ours to draw once DWM has been told not to. Every
            // later resize goes through WM_WINDOWPOSCHANGING; this is the
            // window's first region.
            let mut wr = RECT::default();
            if GetWindowRect(hwnd, &mut wr) != 0 {
                apply_round_region(hwnd, wr.right - wr.left, wr.bottom - wr.top);
            }

            let dark_mode: u32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark_mode as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            let color_none: u32 = DWMWA_COLOR_NONE;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &color_none as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            let bg_color: u32 = 0x001B1818;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_CAPTION_COLOR,
                &bg_color as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }

    // ponytail: KNOWN GLITCH, accepted. Moving the top edge — a drag, or a
    // titlebar peek growing the frame — shows the desktop through the newly
    // exposed strip for one presented frame. DWM presents the new geometry
    // before ANY paint of ours reaches the surface.
    //
    // Kept because each one removes a real part of it: the WVR_VALIDRECTS
    // return from WM_NCCALCSIZE (content keeps its screen position, so the old
    // pixels no longer slide and only the added strip is invalid), `bRedraw: 0`
    // on SetWindowRgn (a region redraw invalidates the whole window and undoes
    // that), and the --color-bg fill in WM_ERASEBKGND, whose colour the webview
    // itself reports through `set_window_background`.
    //
    // Ruled out BY MEASUREMENT, do not retry:
    //  - the round-rect region: the strip is there with SetWindowRgn removed
    //    entirely, and it predates the region;
    //  - the erase colour: traced as 0x121212 against a page rendering
    //    rgb(18, 18, 18) — it already matches, a magenta erase proved the fill
    //    lands, and the desktop frame precedes it regardless;
    //  - the WebView2 surface: an opaque cyan webview background never showed,
    //    so the gap is parent client area, not webview content;
    //  - a scoped InvalidateRect + UpdateWindow of the exposed strip: the
    //    second invalidate flickered the text as well.
    //
    // Why LunaTranslator does not have it, for anyone comparing: it hosts
    // WebView2 as a child of a Qt widget, so its top-level paints its own
    // opaque background through Qt's backing store in the same present as the
    // geometry change. Here the webview owns the whole client and
    // WM_ERASEBKGND is the only parent-side painter. The structural fixes are
    // therefore to stop resizing on hover (shift the content, or reserve the
    // band row permanently) or to put an opaque filler child behind the
    // webview; DwmExtendFrameIntoClientArea, which LunaTranslator also
    // exposes, brings back the drop shadow e45d55f removed.
    unsafe extern "system" fn frameless_subclass_proc(
        hwnd: *mut c_void,
        msg: u32,
        wparam: usize,
        lparam: isize,
        _uid: usize,
        _ref_data: usize,
    ) -> isize {
        match msg {
            // tao keeps its own model of the window flags and rewrites
            // GWL_STYLE wholesale whenever one of them changes — showing the
            // window is enough — which puts WS_CAPTION and WS_THICKFRAME back
            // and the DWM shadow with them. Editing the incoming style is what
            // makes `frameless_style` stick for the window's whole life.
            WM_STYLECHANGING => {
                if wparam as isize == GWL_STYLE as isize {
                    let styles = &mut *(lparam as *mut STYLESTRUCT);
                    styles.style_new = frameless_style(styles.style_new);
                }
            }
            WM_NCCALCSIZE => {
                if wparam != 0 {
                    let params = &mut *(lparam as *mut NCCALCSIZE_PARAMS);
                    if IsZoomed(hwnd) != 0 {
                        let dpi = GetDpiForWindow(hwnd);
                        let dpi_val = if dpi == 0 { 96 } else { dpi };
                        let frame_x = get_system_metrics(SM_CXSIZEFRAME, dpi_val)
                            + get_system_metrics(SM_CXPADDEDBORDER, dpi_val);
                        let frame_y = get_system_metrics(SM_CYSIZEFRAME, dpi_val)
                            + get_system_metrics(SM_CXPADDEDBORDER, dpi_val);

                        params.rgrc[0].left += frame_x;
                        params.rgrc[0].top += frame_y;
                        params.rgrc[0].right -= frame_x;
                        params.rgrc[0].bottom -= frame_y;
                    }
                    // Pin the blit rather than let Windows anchor it at the
                    // client top-left: with the top edge moving — every
                    // titlebar peek, every top-edge drag — that anchor drags
                    // the whole client upward on screen and leaves the pixels
                    // it could not fill along the bottom, where the desktop
                    // shows through until WebView2 repaints. Naming the
                    // surviving rect as both source and destination keeps the
                    // content still and invalidates only the added strip.
                    let old_client = params.rgrc[2];
                    let valid = valid_rect(old_client, params.rgrc[0]);
                    if valid.right <= valid.left || valid.bottom <= valid.top {
                        return WVR_REDRAW;
                    }
                    params.rgrc[1] = valid;
                    params.rgrc[2] = valid;
                    return WVR_VALIDRECTS;
                }
            }
            WM_NCHITTEST => {
                let dpi = GetDpiForWindow(hwnd);
                let dpi_val = if dpi == 0 { 96 } else { dpi as i32 };

                let mut wr = RECT::default();
                if GetWindowRect(hwnd, &mut wr) != 0 {
                    let x = (lparam as i32) as i16 as i32;
                    let y = ((lparam >> 16) as i32) as i16 as i32;

                    // Everything below measures from where the window LOOKS
                    // like it starts. At rest the reserved row is clipped out of
                    // the region, so it receives no input at all — the pointer
                    // goes to whatever is behind it — and the top resize edge
                    // and the drag band belong at the first visible row instead.
                    let top = visual_top(wr.top, dpi_val);

                    let border_px = mul_div(6, dpi_val, 96);
                    let corner_px = mul_div(12, dpi_val, 96);
                    let band_px = mul_div(super::BAND_HEIGHT_LOGICAL as i32, dpi_val, 96);
                    let buttons_width_px = mul_div(120, dpi_val, 96);

                    let is_maximized = IsZoomed(hwnd) != 0;

                    if !is_maximized {
                        if y < top + corner_px && x < wr.left + corner_px {
                            return HTTOPLEFT;
                        }
                        if y < top + corner_px && x >= wr.right - corner_px {
                            return HTTOPRIGHT;
                        }
                        if y >= wr.bottom - corner_px && x < wr.left + corner_px {
                            return HTBOTTOMLEFT;
                        }
                        if y >= wr.bottom - corner_px && x >= wr.right - corner_px {
                            return HTBOTTOMRIGHT;
                        }

                        if y < top + border_px {
                            return HTTOP;
                        }
                        if y >= wr.bottom - border_px {
                            return HTBOTTOM;
                        }
                        if x < wr.left + border_px {
                            return HTLEFT;
                        }
                        if x >= wr.right - border_px {
                            return HTRIGHT;
                        }
                    }

                    if y < top + band_px {
                        if x >= wr.right - buttons_width_px {
                            return HTCLIENT;
                        }
                        return HTCAPTION;
                    }
                }
                return HTCLIENT;
            }
            // A frameless window's client area starts out transparent and the
            // webview paints it a frame or two later, which a resize makes
            // visible. Painting the theme background here fills that gap with
            // the colour the webview is about to draw anyway. It does not
            // remove the top-edge glitch, only shrink it.
            // `WM_NCCALCSIZE` above hands the client area the whole window, but
            // it does not stop `DefWindowProc` from PAINTING a non-client frame
            // on top. `WS_CAPTION` is gone, yet `WS_SYSMENU` and the min/max
            // boxes remain, and what the default handler draws for that is a
            // classic caption — title text, buttons, and the system's
            // horizontal `COLOR_GRADIENTACTIVECAPTION` wash — into the top of
            // the window, which is exactly the reserved band row.
            //
            // It was invisible until per-pixel alpha shipped. GDI writes those
            // pixels through a COLORREF, so their alpha byte is zero: invalid
            // premultiplied data that DWM composites as a colour that shifts
            // with whatever is behind the window. An opaque webview covered
            // them completely; a see-through one does not, and the band is
            // where they land. Diagnosed by suppressing these two messages and
            // watching the gradient go.
            //
            // Nothing legitimate paints here — the client owns every pixel —
            // so `WM_NCPAINT` draws nothing at all.
            WM_NCPAINT => return 0,
            // `-1` is the documented way to keep `DefWindowProc`'s activation
            // bookkeeping while skipping the non-client repaint it would
            // otherwise do, which paints the same caption by a second route.
            WM_NCACTIVATE => return DefSubclassProc(hwnd, msg, wparam, -1),
            WM_ERASEBKGND => {
                // Skipped once the surface is see-through. `CreateSolidBrush`
                // takes a COLORREF, which carries no alpha, so GDI writes zero
                // into the alpha byte — and under the blur-behind DWM applies
                // to a transparent window, zero is fully transparent. Filling
                // would paint a hole rather than a background.
                //
                // Declining here is correct only because `tauri.conf.json`
                // declares no `backgroundColor` today: tao's own
                // `WM_ERASEBKGND` handler fills only when one is configured,
                // and otherwise registers the window class with a NULL
                // background brush. If a `backgroundColor` were ever added to
                // the config, tao would fill with an opaque brush on every
                // erase and quietly defeat this gate.
                if !BACKGROUND_OPAQUE.load(Ordering::Relaxed) {
                    return DefSubclassProc(hwnd, msg, wparam, lparam);
                }
                let mut cr = RECT::default();
                if GetClientRect(hwnd, &mut cr) != 0 {
                    let brush = CreateSolidBrush(BACKGROUND.load(Ordering::Relaxed));
                    if !brush.is_null() {
                        FillRect(wparam as *mut c_void, &cr, brush);
                        DeleteObject(brush);
                    }
                }
                return 1;
            }
            // The region is in window coordinates, so it goes stale the moment
            // the window changes size. This arm runs *before* the new size is
            // applied and sizes the region from the pending WINDOWPOS, so the
            // region and the window change together in one frame — WM_SIZE and
            // WM_WINDOWPOSCHANGED both arrive too late for that.
            WM_WINDOWPOSCHANGING => {
                let pos = &*(lparam as *const WINDOWPOS);
                if pos.flags & SWP_NOSIZE == 0 {
                    apply_round_region(hwnd, pos.cx, pos.cy);
                }
            }
            WM_DPICHANGED => {
                apply_dwm_attributes(hwnd);
            }
            _ => {}
        }
        DefSubclassProc(hwnd, msg, wparam, lparam)
    }

    /// Overrides the erase colour with what the webview actually renders, and
    /// whether that background is opaque.
    ///
    /// The `window.theme()` guess in `init_window` is only a starting value: it
    /// reads the *system* theme, while the page can be on the other one — a
    /// `data-theme` override, or simply a theme change the backend never saw.
    /// A strip erased in the wrong theme's colour is the whole visible glitch,
    /// so the renderer gets to name its own background. `opaque` is read by the
    /// `WM_ERASEBKGND` arm to decide whether to fill at all.
    pub fn set_background(colorref: u32, opaque: bool) {
        BACKGROUND.store(colorref, Ordering::Relaxed);
        BACKGROUND_OPAQUE.store(opaque, Ordering::Relaxed);
    }

    /// Shows or hides the band's reserved row by changing the region alone.
    ///
    /// This is the entire peek on Windows. No `SetWindowPos`, no size change, no
    /// layout change in the page — the row is reserved permanently and the
    /// webview has already painted it, so the reveal is a clip change over
    /// pixels that are ready.
    pub fn set_strip_shown(window: &Window, shown: bool) {
        if STRIP_SHOWN.swap(shown, Ordering::SeqCst) == shown {
            return;
        }
        let Ok(hwnd) = window.hwnd() else { return };
        let hwnd = hwnd.0;
        unsafe {
            let mut wr = RECT::default();
            if GetWindowRect(hwnd, &mut wr) != 0 {
                apply_round_region(hwnd, wr.right - wr.left, wr.bottom - wr.top);
            }
        }
    }

    pub fn init_window(window: &Window) {
        // A theme read that fails leaves the dark default: this app forces the
        // dark immersive title bar below, so dark is the safer guess. Either
        // way `set_background` replaces it as soon as the page has painted.
        if let Ok(theme) = window.theme() {
            BACKGROUND.store(
                match theme {
                    tauri::Theme::Light => BACKGROUND_LIGHT,
                    _ => BACKGROUND_DARK,
                },
                Ordering::Relaxed,
            );
        }
        if let Ok(hwnd) = window.hwnd() {
            let hwnd_val = hwnd.0;
            if !SUBCLASSED.swap(true, Ordering::SeqCst) {
                unsafe {
                    SetWindowSubclass(hwnd_val, frameless_subclass_proc, 1, 0);
                }
            }
            apply_dwm_attributes(hwnd_val);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_valid_rect_is_what_the_old_and_new_client_share() {
            let old = RECT {
                left: 100,
                top: 200,
                right: 700,
                bottom: 600,
            };

            // A 28px grow at the top: every old pixel survives, so the blit
            // keeps all of them and only the added strip is left invalid.
            let grown = RECT {
                top: old.top - 28,
                ..old
            };
            let valid = valid_rect(old, grown);
            assert_eq!(
                (valid.left, valid.top, valid.right, valid.bottom),
                (old.left, old.top, old.right, old.bottom)
            );

            // The shrink back: the survivors are the smaller new client, so
            // nothing is left invalid at all.
            let valid = valid_rect(grown, old);
            assert_eq!(
                (valid.left, valid.top, valid.right, valid.bottom),
                (old.left, old.top, old.right, old.bottom)
            );

            // A move further than the window's own width keeps nothing, and the
            // caller falls back to a full redraw.
            let far = RECT {
                left: old.left + 2000,
                right: old.right + 2000,
                ..old
            };
            let valid = valid_rect(old, far);
            assert!(valid.right <= valid.left, "no pixel survives");
        }

        #[test]
        fn frameless_style_drops_the_caption_and_keeps_the_sizing_frame() {
            // The style Tauri hands us on a decorated main window.
            let decorated = 0x04CF_0000u32;
            let frameless = frameless_style(decorated);

            assert_eq!(frameless & WS_CAPTION, 0, "caption draws native chrome");
            assert_eq!(
                frameless & WS_THICKFRAME,
                WS_THICKFRAME,
                "DefWindowProc refuses to resize a window without a sizing frame"
            );
            assert_eq!(
                frameless & !(WS_CAPTION | WS_THICKFRAME),
                decorated & !(WS_CAPTION | WS_THICKFRAME),
                "no other style bit may change"
            );
        }
    }
}

/// The logical height in pixels of the titlebar band on Windows.
pub const BAND_HEIGHT_LOGICAL: u32 = 28;

/// The creation size in `tauri.conf.json`, so a first launch with no saved
/// geometry still gets the reserved row added to its height.
pub const DEFAULT_WINDOW_WIDTH: u32 = 720;
pub const DEFAULT_WINDOW_HEIGHT: u32 = 480;

/// Resolves the requested mode through `supported()` and persists it.
///
/// Split out of `set_chrome_mode` — which needs a Tauri `AppHandle` to reach
/// the window, and on Windows that `AppHandle` has to be concretely `Wry`
/// because `win32_frameless::init_window` is not generic over `Runtime` —
/// purely so this defence can be exercised with a bare `SettingsState` and no
/// app handle at all. Same reason `push_input` above is split out of
/// `set_input`.
fn resolve_chrome_mode(mode: ChromeMode, settings: &SettingsState) -> Result<ChromeMode, String> {
    // Resolved before it is applied or persisted, so an out-of-date webview
    // cannot write a mode this platform will not honour.
    let mode = crate::settings::supported(mode, crate::settings::native_supported());
    settings
        .update(|s| s.chrome_mode = Some(mode))
        .map_err(|e| e.to_string())?;
    Ok(mode)
}

/// The per-platform half of `set_chrome_mode`: everything after the mode has
/// been resolved and persisted.
///
/// Shared with `layout::restore`, which resolves and persists a whole slot and
/// then needs exactly this. Extracted rather than duplicated so the three-way
/// platform split stays in one place.
pub(crate) fn apply_chrome(window: &tauri::Window, mode: ChromeMode) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    apply_decorations_macos(window, mode == ChromeMode::Native)?;
    #[cfg(target_os = "windows")]
    {
        // No `grow_top` here any more. It existed to shed and regain the
        // reserved row across the native boundary, and `supported` means
        // Windows never crosses it — `header` and `content` reserve the row
        // identically, so switching between them must move nothing.
        win32_frameless::init_window(window);
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let _ = window.set_decorations(mode == ChromeMode::Native);
    }
    // Windows alone reads nothing from `mode`: the reserved row is identical
    // in `header` and `content`.
    let _ = mode;
    Ok(())
}

#[tauri::command]
pub fn set_chrome_mode(
    mode: ChromeMode,
    app: tauri::AppHandle,
    settings: State<'_, Arc<SettingsState>>,
) -> Result<(), String> {
    let mode = resolve_chrome_mode(mode, &settings)?;
    apply_chrome(&chrome_target(&app)?, mode)
}

/// Shows or hides the native chrome *without* touching the persisted flag.
/// `set_chrome_mode` cannot serve the hover reveal: it writes
/// `settings.chrome_mode`, so resting the pointer on the titlebar band would
/// quietly turn the title bar back on for the next launch.
/// Whether a peek is currently showing chrome the persisted setting hides.
///
/// Kept here rather than only in the webview because the frame growth below is
/// relative: a duplicate show would grow the window twice, and a webview reload
/// mid-peek would otherwise leave the frontend's copy of the flag out of step
/// with the real frame. One main window, so one flag.
static PEEKED: AtomicBool = AtomicBool::new(false);

/// Whether a peek is showing right now.
///
/// Read by `layout::bind`, which has to correct a box captured mid-peek. The
/// flag stays private so `peek_titlebar` remains its only writer.
pub(crate) fn is_peeked() -> bool {
    PEEKED.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

#[tauri::command]
pub fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

/// Reports whether `peek_titlebar` grows the window frame upward on this
/// platform to absorb the revealed titlebar band.
///
/// Windows does not: it reserves the row permanently and clips it away instead,
/// so there is nothing for `peek-offset` to compensate for and adding it would
/// shift the content it exists to hold still.
#[tauri::command]
pub fn peek_grows_frame() -> bool {
    cfg!(target_os = "macos")
}

#[tauri::command]
pub fn peek_titlebar(visible: bool, height: f64, app: tauri::AppHandle) -> Result<(), String> {
    // Idempotent by contract: only a real transition touches the window.
    if PEEKED.swap(visible, Ordering::SeqCst) == visible {
        return Ok(());
    }
    let window = chrome_target(&app)?;
    // The band is added ON TOP of the window rather than covering the first
    // line, but the two platforms get there differently now. macOS still grows
    // the frame upward and relies on the frontend's `peek-offset` padding to
    // hold the content still. Windows reserves the row permanently and only
    // changes the region, which took the flash from 39% of reveals to 1% —
    // growing the frame made the reveal race a page-wide reflow.
    #[cfg(target_os = "windows")]
    {
        let _ = height;
        win32_frameless::set_strip_shown(&window, visible);
    }
    #[cfg(target_os = "macos")]
    {
        set_titlebar_chrome(&window, visible)?;
        let scale = window.scale_factor().map_err(|e| e.to_string())?;
        let size = window
            .outer_size()
            .map_err(|e| e.to_string())?
            .to_logical::<f64>(scale);
        let pos = window
            .outer_position()
            .map_err(|e| e.to_string())?
            .to_logical::<f64>(scale);
        let delta = if visible { height } else { -height };
        window
            .set_size(tauri::LogicalSize::new(size.width, size.height + delta))
            .map_err(|e| e.to_string())?;
        window
            .set_position(tauri::LogicalPosition::new(pos.x, pos.y - delta))
            .map_err(|e| e.to_string())?;
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let _ = (window, height);
    }
    Ok(())
}

/// A COLORREF (0x00BBGGRR) for an sRGB triple.
fn colorref(r: u8, g: u8, b: u8) -> u32 {
    (b as u32) << 16 | (g as u32) << 8 | r as u32
}

/// The webview reporting the background colour it renders, and whether that
/// background is fully opaque.
///
/// The colour is so the frame growth on a peek erases newly exposed pixels in
/// what the page draws rather than in a colour guessed from the system theme.
/// `opaque` is the *effective* alpha — `native` renders fully opaque whatever
/// is persisted — and gates the fill entirely, for the reason the
/// `WM_ERASEBKGND` arm gives.
#[tauri::command]
pub fn set_window_background(r: u8, g: u8, b: u8, opaque: bool) {
    let color = colorref(r, g, b);
    #[cfg(target_os = "windows")]
    win32_frameless::set_background(color, opaque);
    let _ = (color, opaque);
}

#[tauri::command]
pub fn update_clip_region(
    app: tauri::AppHandle,
    settings: State<'_, Arc<SettingsState>>,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let window = chrome_target(&app)?;
        win32_frameless::init_window(&window);
    }
    let _ = (app, settings);
    Ok(())
}

#[tauri::command]
pub fn minimize_window<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    chrome_target(&app)?.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_maximize_window<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    let window = chrome_target(&app)?;
    if window.is_maximized().map_err(|e| e.to_string())? {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn close_window<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    chrome_target(&app)?.close().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_window_geometry(
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    settings: State<'_, Arc<SettingsState>>,
) -> Result<(), String> {
    settings
        .update(|s| {
            s.window_width = Some(width);
            s.window_height = Some(height);
            s.window_x = Some(x);
            s.window_y = Some(y);
        })
        .map_err(|e| e.to_string())
}

/// The height to give the window when nothing has been saved yet.
///
/// The reserved row is part of the window's real box and is persisted that way —
/// converting between a "visible" and a "real" box on every save and restore was
/// tried first and is a drift hazard: `save_window_geometry` is a fixed point
/// today (saving a saved value changes nothing), and subtracting a band on each
/// save breaks that the moment anything re-saves a stored value. So the only
/// place the band is added is here, once, when there is no saved height to
/// honour.
///
/// No mode parameter: the row is reserved on Windows and nowhere else, and on
/// Windows every reachable mode reserves it (spec §4.4).
pub fn initial_height(saved: Option<u32>) -> u32 {
    match saved {
        Some(h) => h,
        None if cfg!(target_os = "windows") => DEFAULT_WINDOW_HEIGHT + BAND_HEIGHT_LOGICAL,
        None => DEFAULT_WINDOW_HEIGHT,
    }
}

/// The merge `save_settings` performs, without the state wrapper.
///
/// Split out for the reason `resolve_chrome_mode` is: a `#[tauri::command]`
/// taking `State<'_, _>` cannot be called from a test, and the rule this
/// function encodes — which keys are merged, which are assigned, and which are
/// not touched at all — is worth pinning.
///
/// `layout_slots` is deliberately absent. The settings window never sends it,
/// so it arrives as an empty map on every save; assigning it would wipe every
/// bound slot.
pub(crate) fn merge_saved(current: &mut Settings, incoming: Settings) {
    if incoming.furigana_mode.is_some() {
        current.furigana_mode = incoming.furigana_mode;
    }
    if incoming.hide_pos.is_some() {
        current.hide_pos = incoming.hide_pos;
    }
    if incoming.hide_xrefs.is_some() {
        current.hide_xrefs = incoming.hide_xrefs;
    }
    if incoming.hide_usage.is_some() {
        current.hide_usage = incoming.hide_usage;
    }
    if incoming.hide_kanji.is_some() {
        current.hide_kanji = incoming.hide_kanji;
    }
    // Merged like the toggles above rather than assigned like the sizes below:
    // a range input cannot be cleared, so alpha has no "absent means reset"
    // state, and an unguarded assign would only give a future caller a way to
    // wipe the value by omission.
    if incoming.surface_alpha.is_some() {
        current.surface_alpha = incoming.surface_alpha;
    }
    if incoming.text_outline.is_some() {
        current.text_outline = incoming.text_outline;
    }
    // Assigned, like the sizes below: a colour's Reset and a font select's
    // Default both reach here as the key gone, and only an assign can carry
    // "unset" to disk. The settings window always sends all eight.
    current.colour_kanji = incoming.colour_kanji;
    current.colour_kana = incoming.colour_kana;
    current.colour_particle = incoming.colour_particle;
    current.colour_counter = incoming.colour_counter;
    current.colour_furigana = incoming.colour_furigana;
    current.font_family_sentence = incoming.font_family_sentence;
    current.font_family_furigana = incoming.font_family_furigana;
    current.font_family_gloss = incoming.font_family_gloss;
    // Assigned rather than merged, unlike the toggles above: an absent size
    // means the user cleared that box and wants the stylesheet default back,
    // which a merge could never express. The settings window is the only
    // caller and always sends all three.
    current.font_sentence = incoming.font_sentence;
    current.font_gloss = incoming.font_gloss;
    current.furigana_scale = incoming.furigana_scale;
}

#[tauri::command]
pub fn save_settings(
    settings: Settings,
    state: State<'_, Arc<SettingsState>>,
) -> Result<(), String> {
    state
        .update(|s| merge_saved(s, settings))
        .map_err(|e| e.to_string())
}

/// The settings window's label. Named by this module and by `main`'s setup, so
/// it lives here as one constant rather than repeated literals.
pub const SETTINGS_LABEL: &str = "settings";

/// Build the hidden settings window. Called once from `main`'s `setup`.
///
/// Eager, hidden creation for the same reason as the popover (see that
/// module's header): building a webview costs hundreds of milliseconds. It is
/// worse here than a plain delay — the page's stylesheets are imported by
/// `settings.ts`, so under the dev server they arrive as module requests
/// *after* the document itself finishes loading. A window created per open is
/// therefore mapped and unstyled for about a second, showing the webview's
/// default white before the theme lands. Shown and hidden, never rebuilt,
/// there is nothing left to load at reveal time.
pub fn create_settings_window<R: tauri::Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    let window =
        WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App("settings.html".into()))
            .title("ClipYomi Settings")
            .inner_size(360.0, 480.0)
            .min_inner_size(300.0, 400.0)
            .resizable(false)
            .decorations(true)
            .always_on_top(true)
            .visible(false)
            .build()?;

    // Closing hides rather than destroys. A destroyed window would have to be
    // rebuilt on the next open, reintroducing exactly the load this eager
    // build exists to avoid.
    let on_close = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = on_close.hide();
        }
    });

    Ok(())
}

#[tauri::command]
pub fn open_settings_window<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    let window = app
        .get_webview_window(SETTINGS_LABEL)
        .ok_or_else(|| "settings window not found".to_string())?;
    // Not fatal if it was never minimised; the show below is what matters.
    let _ = window.unminimize();
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(settings: State<'_, Arc<SettingsState>>) -> Settings {
    settings.snapshot()
}

/// Called once by the webview immediately after both `parse-result` and
/// `parse-error` listeners finish registering. Lifts the gate that keeps
/// `clipboard::run_poll` from reading the clipboard before anything is
/// listening for the result — see `clipboard::wait_for_frontend`.
#[tauri::command]
pub fn frontend_ready(ready: State<'_, FrontendReady>) {
    ready.0.notify_one();
}

/// The startup error, or `null` when startup succeeded.
///
/// Takes `State<'_, StartupFailure>` rather than `Option<State<'_, StartupFailure>>`:
/// the latter is not a valid Tauri 2 command parameter, since only `State`
/// itself implements `CommandArg`, not `Option<State<_>>`. `main`'s `setup`
/// manages `StartupFailure` unconditionally instead, empty string meaning
/// success.
#[tauri::command]
pub fn startup_error(failure: State<'_, StartupFailure>) -> Option<String> {
    if failure.0.is_empty() {
        None
    } else {
        Some(failure.0.clone())
    }
}

/// A settings-load warning (e.g. a corrupt `settings.json`), or `null` when
/// settings loaded cleanly. Unlike `startup_error`, this is never fatal — see
/// `SettingsWarning`'s doc comment for why the two are separate.
#[tauri::command]
pub fn settings_warning(warning: State<'_, SettingsWarning>) -> Option<String> {
    if warning.0.is_empty() {
        None
    } else {
        Some(warning.0.clone())
    }
}

/// The hotkey registration warning, or `None` when every key took.
#[tauri::command]
pub fn shortcut_warning(warning: State<'_, ShortcutWarning>) -> Option<String> {
    if warning.0.is_empty() {
        None
    } else {
        Some(warning.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::Manager;

    /// `set_input` is a thin push into the watch channel; the part worth testing
    /// is that the newest value is what a reader sees.
    #[tokio::test]
    async fn set_input_publishes_the_text_to_the_channel() {
        let (tx, mut rx) = tokio::sync::watch::channel(String::new());
        push_input(&tx, "東京".to_string()).expect("push");
        assert_eq!(
            crate::parse::next_input(&mut rx).await.as_deref(),
            Some("東京")
        );
    }

    /// A dead worker must surface as an error rather than a silent no-op: the
    /// user pressed Parse and nothing would ever arrive.
    #[tokio::test]
    async fn set_input_reports_a_dead_worker() {
        let (tx, rx) = tokio::sync::watch::channel(String::new());
        drop(rx);
        assert!(push_input(&tx, "東京".to_string()).is_err());
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ta-commands-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// The reserved row is Windows-only and unconditional there now: `native`
    /// does not exist on that platform, and `header` and `content` both reserve
    /// the row (spec §4.4). Saved values are the real box already, so they pass
    /// through untouched — that is what keeps `save_window_geometry` a fixed
    /// point.
    #[test]
    fn initial_height_reserves_the_band_row_only_on_a_first_windows_launch() {
        assert_eq!(initial_height(Some(508)), 508);
        if cfg!(target_os = "windows") {
            assert_eq!(
                initial_height(None),
                DEFAULT_WINDOW_HEIGHT + BAND_HEIGHT_LOGICAL
            );
        } else {
            assert_eq!(initial_height(None), DEFAULT_WINDOW_HEIGHT);
        }
    }

    #[test]
    fn peek_does_not_grow_the_frame_on_windows() {
        // The whole point of the redesign: the row is reserved, so the frame
        // stays put and `peek-offset` must never be applied here.
        if cfg!(target_os = "windows") {
            assert!(!peek_grows_frame());
        }
        if cfg!(target_os = "macos") {
            assert!(peek_grows_frame());
        }
    }

    #[test]
    fn save_window_geometry_updates_settings_state() {
        let dir = scratch("save-geometry");
        let state = Arc::new(SettingsState::new(
            dir.join("settings.json"),
            Settings::default(),
        ));
        let app = tauri::test::mock_app();
        app.manage(Arc::clone(&state));
        let state_arg = app.state::<Arc<SettingsState>>();
        save_window_geometry(600, 200, 50, 80, state_arg).unwrap();
        let s = state.snapshot();
        assert_eq!(s.window_width, Some(600));
        assert_eq!(s.window_height, Some(200));
        assert_eq!(s.window_x, Some(50));
        assert_eq!(s.window_y, Some(80));
    }

    /// Clearing a size box in the settings window sends a snapshot with the key
    /// gone, and that has to reach disk as "unset" — otherwise the old size
    /// comes back on the next launch and the box cannot be cleared at all.
    /// Safe because the settings window is the only caller, and it always sends
    /// a full snapshot of all three.
    #[test]
    fn save_settings_clears_a_font_size_the_payload_omits() {
        let dir = scratch("clear-font-size");
        let state = Arc::new(SettingsState::new(
            dir.join("settings.json"),
            Settings {
                font_sentence: Some(30),
                ..Default::default()
            },
        ));
        let app = tauri::test::mock_app();
        app.manage(Arc::clone(&state));
        let state_arg = app.state::<Arc<SettingsState>>();

        save_settings(Settings::default(), state_arg).unwrap();

        assert_eq!(state.snapshot().font_sentence, None);
    }

    #[test]
    fn save_settings_persists_the_font_sizes() {
        let dir = scratch("save-font-sizes");
        let state = Arc::new(SettingsState::new(
            dir.join("settings.json"),
            Settings::default(),
        ));
        let app = tauri::test::mock_app();
        app.manage(Arc::clone(&state));
        let state_arg = app.state::<Arc<SettingsState>>();
        let settings = Settings {
            font_sentence: Some(30),
            font_gloss: Some(18),
            furigana_scale: Some(70),
            ..Default::default()
        };
        save_settings(settings, state_arg).unwrap();
        let s = state.snapshot();
        assert_eq!(s.font_sentence, Some(30));
        assert_eq!(s.font_gloss, Some(18));
        assert_eq!(s.furigana_scale, Some(70));
    }

    #[test]
    fn save_settings_updates_furigana_mode_in_settings_state() {
        let dir = scratch("save-settings");
        let state = Arc::new(SettingsState::new(
            dir.join("settings.json"),
            Settings::default(),
        ));
        let app = tauri::test::mock_app();
        app.manage(Arc::clone(&state));
        let state_arg = app.state::<Arc<SettingsState>>();
        let settings = Settings {
            furigana_mode: Some("romaji".to_string()),
            ..Default::default()
        };
        save_settings(settings, state_arg).unwrap();
        let s = state.snapshot();
        assert_eq!(s.furigana_mode, Some("romaji".to_string()));
    }

    #[test]
    fn save_settings_updates_gloss_filters_in_settings_state() {
        let dir = scratch("save-settings-filters");
        let state = Arc::new(SettingsState::new(
            dir.join("settings.json"),
            Settings::default(),
        ));
        let app = tauri::test::mock_app();
        app.manage(Arc::clone(&state));
        let state_arg = app.state::<Arc<SettingsState>>();
        let settings = Settings {
            hide_pos: Some(true),
            hide_xrefs: Some(true),
            hide_usage: Some(false),
            hide_kanji: Some(true),
            ..Default::default()
        };
        save_settings(settings, state_arg).unwrap();
        let s = state.snapshot();
        assert_eq!(s.hide_pos, Some(true));
        assert_eq!(s.hide_xrefs, Some(true));
        assert_eq!(s.hide_usage, Some(false));
        assert_eq!(s.hide_kanji, Some(true));
    }

    /// The guard that separates alpha from the font sizes. The sizes are
    /// assigned unguarded, deliberately, so that clearing a box restores the
    /// stylesheet default. A range input cannot be cleared, so alpha has no
    /// "absent means reset" state to express, and an unguarded assign would
    /// only hand a future caller a way to wipe the value by omission.
    #[test]
    fn save_settings_leaves_a_stored_alpha_alone_when_the_payload_omits_it() {
        let dir = scratch("save-settings-alpha");
        let state = Arc::new(SettingsState::new(
            dir.join("settings.json"),
            Settings {
                surface_alpha: Some(40),
                ..Default::default()
            },
        ));
        let app = tauri::test::mock_app();
        app.manage(Arc::clone(&state));

        save_settings(
            Settings {
                surface_alpha: None,
                ..Default::default()
            },
            app.state::<Arc<SettingsState>>(),
        )
        .unwrap();

        assert_eq!(state.snapshot().surface_alpha, Some(40));
    }

    #[test]
    fn save_settings_writes_a_supplied_alpha() {
        let dir = scratch("save-settings-alpha-set");
        let state = Arc::new(SettingsState::new(
            dir.join("settings.json"),
            Settings::default(),
        ));
        let app = tauri::test::mock_app();
        app.manage(Arc::clone(&state));

        save_settings(
            Settings {
                surface_alpha: Some(0),
                ..Default::default()
            },
            app.state::<Arc<SettingsState>>(),
        )
        .unwrap();

        assert_eq!(state.snapshot().surface_alpha, Some(0));
    }

    /// Builds a window the way the real app labels them, so `chrome_target`
    /// has something to resolve.
    fn window(app: &tauri::AppHandle<tauri::test::MockRuntime>, label: &str) {
        WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
            .build()
            .unwrap();
    }

    #[test]
    fn chrome_target_resolves_main_even_when_settings_window_exists() {
        let app = tauri::test::mock_app();
        let h = app.handle().clone();
        window(&h, "main");
        window(&h, "settings");
        // The regression: invoked from the settings webview, these commands
        // used to act on whichever window called them.
        assert_eq!(chrome_target(&h).unwrap().label(), "main");
    }

    #[test]
    fn chrome_target_errors_without_a_main_window() {
        let app = tauri::test::mock_app();
        let h = app.handle().clone();
        window(&h, "settings");
        assert!(chrome_target(&h).is_err());
    }

    /// Spec §8's promised test: `set_chrome_mode` must persist the mode
    /// `supported()` resolves, not the one the webview asked for — otherwise
    /// an out-of-date webview could write a mode this platform will not
    /// honour. Goes through `resolve_chrome_mode` rather than the command
    /// itself: on Windows, `set_chrome_mode` needs a concretely-`Wry`
    /// `AppHandle` to reach `win32_frameless::init_window`, which a mock
    /// runtime cannot supply, so `resolve_chrome_mode` exists precisely to
    /// carry the tested defence without that dependency. Driven off
    /// `native_supported()`, like the migration tests above, so it is
    /// meaningful on either platform: on Windows a `Native` request persists
    /// `Header`; where native chrome exists, it persists `Native`.
    #[test]
    fn set_chrome_mode_persists_the_resolved_mode_not_the_requested_one() {
        let dir = scratch("set-chrome-mode");
        let state = SettingsState::new(dir.join("settings.json"), Settings::default());

        resolve_chrome_mode(ChromeMode::Native, &state).unwrap();

        let expected =
            crate::settings::supported(ChromeMode::Native, crate::settings::native_supported());
        assert_eq!(state.snapshot().chrome_mode, Some(expected));
    }

    #[test]
    fn create_settings_window_builds_it_before_any_open() {
        let app = tauri::test::mock_app();
        create_settings_window(&app).unwrap();
        assert!(app.handle().get_webview_window(SETTINGS_LABEL).is_some());
    }

    #[test]
    fn open_settings_window_reveals_the_pre_created_window() {
        let app = tauri::test::mock_app();
        create_settings_window(&app).unwrap();
        let handle = app.handle().clone();
        assert!(open_settings_window(handle.clone()).is_ok());
        // Repeated opens just re-reveal it; nothing is rebuilt.
        assert!(open_settings_window(handle).is_ok());
    }

    #[test]
    fn open_settings_window_errors_when_setup_never_created_it() {
        let app = tauri::test::mock_app();
        assert!(open_settings_window(app.handle().clone()).is_err());
    }

    #[test]
    fn a_colorref_puts_blue_in_the_high_byte() {
        // 0x00BBGGRR, not RGB: the byte order is the whole point of the helper.
        assert_eq!(colorref(0x12, 0x34, 0x56), 0x0056_3412);
        // The theme colours are neutral greys, which is why the order went
        // unnoticed while they were the only callers.
        assert_eq!(colorref(0x12, 0x12, 0x12), 0x0012_1212);
    }

    #[test]
    fn platform_commands_match_target_os() {
        assert_eq!(is_macos(), cfg!(target_os = "macos"));
        // Windows reserves the band's row permanently and clips it instead of
        // growing the frame, so it reports false here now.
        assert_eq!(peek_grows_frame(), cfg!(target_os = "macos"));
        #[cfg(target_os = "macos")]
        assert_eq!(platform(), "macos");
        #[cfg(target_os = "windows")]
        assert_eq!(platform(), "windows");
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        assert_eq!(platform(), "linux");
    }

    #[test]
    fn geometry_round_trip_is_a_fixed_point_including_at_min_height() {
        let dir = scratch("geometry-round-trip");
        let state = Arc::new(SettingsState::new(
            dir.join("settings.json"),
            Settings::default(),
        ));
        let app = tauri::test::mock_app();
        app.manage(Arc::clone(&state));

        // Test cases: (runtime_w, runtime_h, runtime_x, runtime_y)
        // 1. Standard default window
        // 2. Minimum content height (80) -> runtime height 108
        // 3. Raw min height boundary (80)
        // 4. Zero and negative coordinates
        let test_cases = vec![
            (720, 508, 100, 100),
            (160, 108, 0, 0),
            (160, 80, -50, -20),
            (800, 600, -100, 200),
        ];

        for (w, h, x, y) in test_cases {
            let state_arg = app.state::<Arc<SettingsState>>();
            save_window_geometry(w, h, x, y, state_arg).unwrap();
            let first_saved = state.snapshot();

            let loaded_w = first_saved.window_width.unwrap();
            let loaded_h = first_saved.window_height.unwrap();
            let loaded_x = first_saved.window_x.unwrap();
            let loaded_y = first_saved.window_y.unwrap();

            let runtime_h = loaded_h;
            let runtime_y = loaded_y;

            let state_arg2 = app.state::<Arc<SettingsState>>();
            save_window_geometry(loaded_w, runtime_h, loaded_x, runtime_y, state_arg2).unwrap();
            let second_saved = state.snapshot();

            assert_eq!(second_saved.window_width, first_saved.window_width);
            assert_eq!(
                second_saved.window_height, first_saved.window_height,
                "height fixed point for case ({w},{h},{x},{y})"
            );
            assert_eq!(second_saved.window_x, first_saved.window_x);
            assert_eq!(
                second_saved.window_y, first_saved.window_y,
                "y fixed point for case ({w},{h},{x},{y})"
            );
        }
    }

    #[test]
    fn window_controls_act_on_main_window() {
        let app = tauri::test::mock_app();
        let h = app.handle().clone();
        window(&h, "main");
        assert!(minimize_window(h.clone()).is_ok());
        assert!(toggle_maximize_window(h.clone()).is_ok());
        assert!(close_window(h).is_ok());
    }

    /// The settings window's `currentSnapshot()` (`settings.ts:192`) does not
    /// build `layout_slots`, so it arrives here as serde's default — an empty
    /// map — on every save from that window. Assigning it would wipe every
    /// slot the moment the user changed the theme, and nothing in the code
    /// says "leave it alone", so only this test stops a future contributor
    /// completing the pattern.
    #[test]
    fn a_save_from_the_settings_window_leaves_the_layout_slots_alone() {
        let mut current = crate::settings::Settings::default();
        current.layout_slots.insert(
            "1".to_string(),
            crate::settings::LayoutSlot {
                window_width: 720,
                window_height: 200,
                window_x: 0,
                window_y: 0,
                chrome_mode: ChromeMode::Content,
                always_on_top: false,
                surface_alpha: Some(35),
                font_sentence: None,
                font_gloss: None,
                furigana_scale: None,
                furigana_mode: None,
                extra: serde_json::Map::new(),
            },
        );

        merge_saved(
            &mut current,
            crate::settings::Settings {
                furigana_mode: Some("katakana".to_string()),
                ..Default::default()
            },
        );

        assert_eq!(
            current.layout_slots.len(),
            1,
            "a settings save wiped a slot"
        );
        assert_eq!(current.layout_slots["1"].surface_alpha, Some(35));
    }

    /// The appearance keys follow the font sizes: assigned, so a snapshot
    /// with a key gone reaches disk as "unset" — a colour's Reset and a font
    /// select's Default are both expressed by omission. The outline toggle is
    /// a checkbox, always sent, so it merges like `hide_*`.
    #[test]
    fn save_settings_assigns_the_appearance_keys_and_clears_omitted_ones() {
        let dir = scratch("appearance-keys");
        let state = Arc::new(SettingsState::new(
            dir.join("settings.json"),
            Settings {
                colour_kanji: Some("#000000".into()),
                font_family_sentence: Some("Yu Gothic".into()),
                text_outline: Some(true),
                ..Default::default()
            },
        ));
        let app = tauri::test::mock_app();
        app.manage(Arc::clone(&state));
        let state_arg = app.state::<Arc<SettingsState>>();

        save_settings(
            Settings {
                colour_kana: Some("#ffffff".into()),
                font_family_gloss: Some("Segoe UI".into()),
                ..Default::default()
            },
            state_arg,
        )
        .unwrap();

        let s = state.snapshot();
        assert_eq!(s.colour_kanji, None);
        assert_eq!(s.colour_kana, Some("#ffffff".into()));
        assert_eq!(s.font_family_sentence, None);
        assert_eq!(s.font_family_gloss, Some("Segoe UI".into()));
        assert_eq!(
            s.text_outline,
            Some(true),
            "a toggle merges; omission never clears it"
        );
    }
}
