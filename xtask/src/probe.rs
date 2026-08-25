// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! THROWAWAY PROBE (spike, revert me): samples one strip of the screen as fast
//! as `BitBlt` allows, stamping every sample with `QueryPerformanceCounter`.
//!
//! It exists because `ffmpeg -f gdigrab` was measured to be useless as a clock:
//! asked for 240fps on a 240x42 strip it ignored the rate and re-emitted a
//! cached grab — 2,768,710,656 bytes, 1.8M frames, for a nominal 3 seconds. A
//! frame *index* carried no time information, so nothing downstream could tell
//! a one-refresh flash from a duplicated buffer. Every sample here carries its
//! own timestamp instead.
//!
//! Prints `# freq=<qpc frequency>` then `ticks,r,g,b` per sample on stdout, all
//! of it after the loop — formatting inside the loop would be the slowest thing
//! in it. The ticks are raw and absolute: `QueryPerformanceCounter` is a
//! system-wide clock, so the app's reveal log and these samples land on the
//! same timeline with nothing to align.
//!
//! One sample per display refresh is the ceiling. `BitBlt` against the screen
//! DC was measured to block on the compositor: 2882 samples in 20.0s = 144Hz
//! against a 143Hz panel, median gap 6.95ms. Asking for more does not help.

use std::ffi::c_void;

const SRCCOPY: u32 = 0x00CC_0020;
const DIB_RGB_COLORS: u32 = 0;
const BI_RGB: u32 = 0;
/// `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`. Without it Windows virtualizes
/// our screen coordinates and the strip we blit is not the strip the app
/// reported.
const PER_MONITOR_AWARE_V2: isize = -4;
/// Every 4th pixel. The whole strip changes state at once, so a quarter of them
/// carries the same signal and the loop stays tight.
const PIXEL_STRIDE: usize = 4;

#[repr(C)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_ppm: i32,
    y_ppm: i32,
    clr_used: u32,
    clr_important: u32,
}

#[link(name = "user32")]
extern "system" {
    fn GetDC(hwnd: *mut c_void) -> *mut c_void;
    fn ReleaseDC(hwnd: *mut c_void, hdc: *mut c_void) -> i32;
    fn SetProcessDpiAwarenessContext(ctx: isize) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(hdc: *mut c_void) -> *mut c_void;
    fn DeleteDC(hdc: *mut c_void) -> i32;
    fn CreateDIBSection(
        hdc: *mut c_void,
        bmi: *const BitmapInfoHeader,
        usage: u32,
        bits: *mut *mut c_void,
        section: *mut c_void,
        offset: u32,
    ) -> *mut c_void;
    fn SelectObject(hdc: *mut c_void, obj: *mut c_void) -> *mut c_void;
    fn DeleteObject(obj: *mut c_void) -> i32;
    fn BitBlt(
        dst: *mut c_void,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        src: *mut c_void,
        sx: i32,
        sy: i32,
        rop: u32,
    ) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn QueryPerformanceCounter(value: *mut i64) -> i32;
    fn QueryPerformanceFrequency(value: *mut i64) -> i32;
}

#[repr(C)]
#[derive(Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[link(name = "user32")]
extern "system" {
    fn FindWindowW(class: *const u16, title: *const u16) -> *mut c_void;
    fn GetWindowRect(hwnd: *mut c_void, rect: *mut Rect) -> i32;
    fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
    fn GetForegroundWindow() -> *mut c_void;
    fn BringWindowToTop(hwnd: *mut c_void) -> i32;
    fn SetWindowPos(
        hwnd: *mut c_void,
        after: *mut c_void,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
}

const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;

/// Every entry point must call this before it touches a coordinate. Without it
/// Windows virtualizes them: `probe-rect` reported 813x313 for a window that is
/// 1220x470 physical, and an entire appearance run sampled the wrong place and
/// reported 0.0 differences everywhere.
fn make_dpi_aware() {
    unsafe { SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2) };
}

fn app_hwnd() -> Result<*mut c_void, Box<dyn std::error::Error>> {
    let title: Vec<u16> = "ClipYomi".encode_utf16().chain(std::iter::once(0)).collect();
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() {
        return Err("no window titled \"ClipYomi\" is open".into());
    }
    Ok(hwnd)
}

/// THROWAWAY PROBE (spike, revert me): the window's rect, read live.
///
/// Exists because reading it from the file the app writes at startup was
/// measured to be wrong: the window had moved to (2483, 53) while the file
/// still said (1791, 426), so an entire appearance run sampled other windows
/// and reported their pixels as defects. Geometry is asked for at sample time
/// from here on.
pub fn rect() -> Result<(), Box<dyn std::error::Error>> {
    make_dpi_aware();
    let hwnd = app_hwnd()?;
    let mut r = Rect::default();
    if unsafe { GetWindowRect(hwnd, &mut r) } == 0 {
        return Err("GetWindowRect failed".into());
    }
    println!("{},{},{},{}", r.left, r.top, r.right - r.left, r.bottom - r.top);
    Ok(())
}

/// THROWAWAY PROBE (spike, revert me): brings the window to the foreground.
///
/// The appearance checks passed four out of four while missing a defect that is
/// plainly visible, because they only ever sampled an UNFOCUSED window — the
/// move is done with `SWP_NOACTIVATE` and the terminal held focus the whole
/// time. Whatever DWM paints on activation was never in frame. Prints whether
/// the window actually came forward, since a background process is not always
/// allowed to do this.
pub fn activate() -> Result<(), Box<dyn std::error::Error>> {
    make_dpi_aware();
    let hwnd = app_hwnd()?;
    unsafe {
        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd);
        println!("{}", if GetForegroundWindow() == hwnd { "focused" } else { "not-focused" });
    }
    Ok(())
}

/// THROWAWAY PROBE (spike, revert me): moves the window, so the harness can
/// sample the same screen coordinates with the window absent and get an exact
/// reference for "what shows through" instead of guessing from a patch beside
/// it — which produced false failures over wallpaper that varies horizontally.
pub fn move_to() -> Result<(), Box<dyn std::error::Error>> {
    make_dpi_aware();
    let hwnd = app_hwnd()?;
    let (x, y) = (arg(2)?, arg(3)?);
    unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    Ok(())
}

fn arg(n: usize) -> Result<i32, Box<dyn std::error::Error>> {
    Ok(std::env::args()
        .nth(n)
        .ok_or("usage: probe-sample X Y W H SECONDS")?
        .parse()?)
}

/// THROWAWAY PROBE (spike, revert me): one grab of a rect, printed as a mean
/// RGB per row.
///
/// Per row rather than one mean for the whole rect, because the defects being
/// checked are shaped: a tint that ramps down the strip averages out to a
/// plausible-looking colour, and a square corner only differs from a round one
/// in the first few rows.
pub fn grab() -> Result<(), Box<dyn std::error::Error>> {
    let (x, y, w, h) = (arg(2)?, arg(3)?, arg(4)?, arg(5)?);
    if w <= 0 || h <= 0 {
        return Err("width and height must be positive".into());
    }
    unsafe {
        SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2);
        let screen = GetDC(std::ptr::null_mut());
        if screen.is_null() {
            return Err("GetDC(NULL) failed".into());
        }
        let mem = CreateCompatibleDC(screen);
        let bmi = BitmapInfoHeader {
            size: std::mem::size_of::<BitmapInfoHeader>() as u32,
            width: w,
            height: -h,
            planes: 1,
            bit_count: 32,
            compression: BI_RGB,
            size_image: 0,
            x_ppm: 0,
            y_ppm: 0,
            clr_used: 0,
            clr_important: 0,
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(mem, &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if dib.is_null() || bits.is_null() {
            DeleteDC(mem);
            ReleaseDC(std::ptr::null_mut(), screen);
            return Err("CreateDIBSection failed".into());
        }
        SelectObject(mem, dib);
        if BitBlt(mem, 0, 0, w, h, screen, x, y, SRCCOPY) == 0 {
            DeleteObject(dib);
            DeleteDC(mem);
            ReleaseDC(std::ptr::null_mut(), screen);
            return Err("BitBlt failed".into());
        }
        let px = bits as *const u8;
        let mut out = String::new();
        for row in 0..h {
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            for col in 0..w {
                let o = ((row * w + col) * 4) as usize;
                b += *px.add(o) as u32;
                g += *px.add(o + 1) as u32;
                r += *px.add(o + 2) as u32;
            }
            let n = w as u32;
            out.push_str(&format!("{row},{},{},{}\n", r / n, g / n, b / n));
        }
        DeleteObject(dib);
        DeleteDC(mem);
        ReleaseDC(std::ptr::null_mut(), screen);
        print!("{out}");
    }
    Ok(())
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (x, y, w, h, seconds) = (arg(2)?, arg(3)?, arg(4)?, arg(5)?, arg(6)?);
    if w <= 0 || h <= 0 || seconds <= 0 {
        return Err("width, height and seconds must all be positive".into());
    }

    unsafe {
        SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2);

        let screen = GetDC(std::ptr::null_mut());
        if screen.is_null() {
            return Err("GetDC(NULL) failed".into());
        }
        let mem = CreateCompatibleDC(screen);
        if mem.is_null() {
            ReleaseDC(std::ptr::null_mut(), screen);
            return Err("CreateCompatibleDC failed".into());
        }

        // Negative height: top-down rows, so row 0 is the top of the strip.
        let bmi = BitmapInfoHeader {
            size: std::mem::size_of::<BitmapInfoHeader>() as u32,
            width: w,
            height: -h,
            planes: 1,
            bit_count: 32,
            compression: BI_RGB,
            size_image: 0,
            x_ppm: 0,
            y_ppm: 0,
            clr_used: 0,
            clr_important: 0,
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(mem, &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if dib.is_null() || bits.is_null() {
            DeleteDC(mem);
            ReleaseDC(std::ptr::null_mut(), screen);
            return Err("CreateDIBSection failed".into());
        }
        SelectObject(mem, dib);

        let mut freq: i64 = 0;
        QueryPerformanceFrequency(&mut freq);
        if freq == 0 {
            return Err("QueryPerformanceFrequency returned 0".into());
        }
        let mut start: i64 = 0;
        QueryPerformanceCounter(&mut start);
        let deadline = start + freq * seconds as i64;

        let pixels = (w * h) as usize;
        // A little over the measured 144Hz ceiling. Sized for a 17-minute run,
        // where a 10kHz guess would have asked for 326MB.
        let mut out: Vec<(i64, u32, u32, u32)> = Vec::with_capacity(seconds as usize * 200);
        let mut now = start;
        while now < deadline {
            QueryPerformanceCounter(&mut now);
            if BitBlt(mem, 0, 0, w, h, screen, x, y, SRCCOPY) == 0 {
                continue;
            }
            let px = bits as *const u8;
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            let mut i = 0usize;
            while i < pixels {
                let o = i * 4;
                // 32bpp BI_RGB is BGRA in memory.
                b += *px.add(o) as u32;
                g += *px.add(o + 1) as u32;
                r += *px.add(o + 2) as u32;
                i += PIXEL_STRIDE;
            }
            let n = (pixels / PIXEL_STRIDE).max(1) as u32;
            out.push((now, r / n, g / n, b / n));
        }

        DeleteObject(dib);
        DeleteDC(mem);
        ReleaseDC(std::ptr::null_mut(), screen);

        let mut buf = String::with_capacity(out.len() * 24);
        for (ticks, r, g, b) in &out {
            buf.push_str(&format!("{ticks},{r},{g},{b}\n"));
        }
        println!("# freq={freq}");
        print!("{buf}");
    }
    Ok(())
}
