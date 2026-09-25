// SPDX-License-Identifier: MIT OR Apache-2.0

//! Keeps a pinned main window above other pinned windows on Windows.
//!
//! `WS_EX_TOPMOST` puts a window in the topmost band, but inside that band the
//! order is plain z-order, so any other topmost window that is raised later
//! covers this one. There is no "more topmost" for an ordinary app; the window
//! re-asserts `HWND_TOPMOST` at the two moments something else gets raised:
//!
//! - Another process takes the foreground. Activating a pinned window raises
//!   it, so this is the general case — a pinned visual novel, a video player.
//! - Magpie starts scaling, or the game it scales returns to the foreground.
//!   Magpie raises its fullscreen topmost window ~90 ms *after* the foreground
//!   change, so the first case alone still loses to it (measured). It then
//!   broadcasts the registered message `MagpieScalingChanged` with
//!   `wParam == 1`, and documents re-asserting `HWND_TOPMOST` on it as the way
//!   to stay above the scaled window (docs/Interact with Magpie
//!   programally.md); LunaTranslator does the same.

use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;
const WINEVENT_SKIPOWNPROCESS: u32 = 0x0002;
const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOPMOST: i32 = 0x0000_0008;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOACTIVATE: u32 = 0x0010;
/// `MagpieScalingChanged`'s "scaling started, or the source window returned to
/// the foreground". `0` (ended, or lost focus) is left alone: Magpie's sample
/// unpins there, but here being pinned is the user's setting, not Magpie's.
const SCALING_STARTED: usize = 1;

// MSDN's spelling, as in `commands::win32_frameless`.
#[allow(non_snake_case, clippy::upper_case_acronyms)]
type SUBCLASSPROC = unsafe extern "system" fn(
    hWnd: *mut c_void,
    uMsg: u32,
    wParam: usize,
    lParam: isize,
    uIdSubclass: usize,
    dwRefData: usize,
) -> isize;

type WinEventProc = unsafe extern "system" fn(*mut c_void, u32, *mut c_void, i32, i32, u32, u32);

#[allow(non_snake_case)]
extern "system" {
    fn SetWinEventHook(
        eventMin: u32,
        eventMax: u32,
        hmodWinEventProc: *mut c_void,
        pfnWinEventProc: WinEventProc,
        idProcess: u32,
        idThread: u32,
        dwFlags: u32,
    ) -> *mut c_void;
    fn RegisterWindowMessageW(lpString: *const u16) -> u32;
    fn GetWindowLongW(hWnd: *mut c_void, nIndex: i32) -> i32;
    fn SetWindowPos(
        hWnd: *mut c_void,
        hWndInsertAfter: *mut c_void,
        X: i32,
        Y: i32,
        cx: i32,
        cy: i32,
        uFlags: u32,
    ) -> i32;
    fn SetWindowSubclass(
        hWnd: *mut c_void,
        pfnSubclass: SUBCLASSPROC,
        uIdSubclass: usize,
        dwRefData: usize,
    ) -> i32;
    fn DefSubclassProc(hWnd: *mut c_void, uMsg: u32, wParam: usize, lParam: isize) -> isize;
}

/// The main window, for the foreground hook, whose callback is not told it.
static MAIN: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

/// The id `RegisterWindowMessageW` gave `MagpieScalingChanged`; `0` until
/// `start` has run, and `0` is never a registered id.
static MAGPIE_SCALING_CHANGED: AtomicU32 = AtomicU32::new(0);

/// Install both re-raise triggers for `window`. Called once from `main`'s
/// `setup`, which runs on the window's own thread — the thread that pumps its
/// messages. `SetWindowSubclass` requires that, and an out-of-context WinEvent
/// hook delivers its callback to the installing thread, so it needs it too.
///
/// Neither is ever removed: both live as long as the process, as the window
/// does. Either failing is not fatal: the window is still pinned, it just loses
/// to whatever is raised after it, as it always did.
pub fn start(window: &tauri::WebviewWindow) {
    let Ok(hwnd) = window.hwnd() else { return };
    MAIN.store(hwnd.0, Ordering::Relaxed);

    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            std::ptr::null_mut(),
            on_foreground,
            0,
            0,
            // Our own windows taking focus — settings, the popover — are not a
            // reason to re-raise, and skipping them costs nothing.
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    if hook.is_null() {
        eprintln!("could not install the always-on-top foreground hook");
    }

    // The broadcast reaches every top-level window, so the main window receives
    // it without registering anywhere; a clipyomi running elevated above Magpie
    // would not (UIPI), and would need `ChangeWindowMessageFilterEx`.
    let name: Vec<u16> = "MagpieScalingChanged\0".encode_utf16().collect();
    let id = unsafe { RegisterWindowMessageW(name.as_ptr()) };
    if id == 0 {
        eprintln!("could not register MagpieScalingChanged");
        return;
    }
    MAGPIE_SCALING_CHANGED.store(id, Ordering::Relaxed);
    // `uIdSubclass` only has to be unique per proc, and this proc is its own.
    unsafe { SetWindowSubclass(hwnd.0, magpie_subclass_proc, 1, 0) };
}

/// Put `hwnd` back at the top of the topmost band, if it is pinned.
///
/// The window's own style bit is the setting: the toggle, startup and a layout
/// restore all set it, so turning always-on-top off stops this without any of
/// them having to know it exists.
unsafe fn raise_if_pinned(hwnd: *mut c_void) {
    if hwnd.is_null() || GetWindowLongW(hwnd, GWL_EXSTYLE) & WS_EX_TOPMOST == 0 {
        return;
    }
    const HWND_TOPMOST: *mut c_void = -1isize as *mut c_void;
    // No SWP_SHOWWINDOW: a hidden or minimized window stays that way.
    SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
}

// ponytail: a window that re-raises itself while it already has focus, or an
// overlay other than Magpie that raises itself without a broadcast, still
// wins; covering those means watching z-order itself (EVENT_OBJECT_REORDER),
// which needs filters for menus, tooltips and the IME — add it against a real
// app when one turns up.
unsafe extern "system" fn on_foreground(
    _hook: *mut c_void,
    _event: u32,
    _hwnd: *mut c_void,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    raise_if_pinned(MAIN.load(Ordering::Relaxed));
}

unsafe extern "system" fn magpie_subclass_proc(
    hwnd: *mut c_void,
    msg: u32,
    wparam: usize,
    lparam: isize,
    _uid: usize,
    _ref_data: usize,
) -> isize {
    if msg == MAGPIE_SCALING_CHANGED.load(Ordering::Relaxed) && wparam == SCALING_STARTED {
        raise_if_pinned(hwnd);
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}
