use gtk::prelude::*;

pub const DEFAULT_WIDTH: i32 = 1152;
pub const DEFAULT_HEIGHT: i32 = 720;

fn startup_size(width: i32, height: i32) -> (i32, i32) {
    (
        DEFAULT_WIDTH.min((width - 48).max(1)),
        DEFAULT_HEIGHT.min((height - 48).max(1)),
    )
}

fn centered_origin(x: f64, y: f64, width: f64, height: f64, w: f64, h: f64) -> (f64, f64) {
    (
        x + ((width - w) / 2.0).max(0.0),
        y + ((height - h) / 2.0).max(0.0),
    )
}

pub fn prepare(window: &gtk::Window, reference: Option<&gtk::Window>) {
    window.unmaximize();
    window.unfullscreen();
    window.set_default_size(DEFAULT_WIDTH, DEFAULT_HEIGHT);
    WidgetExt::realize(window);
    let reference = reference.unwrap_or(window);
    let available = platform::work_area_size(reference).or_else(|| {
        let surface = reference.surface()?;
        let monitor = surface.display().monitor_at_surface(&surface)?;
        let area = monitor.geometry();
        Some((area.width(), area.height()))
    });
    if let Some((width, height)) = available {
        let (width, height) = startup_size(width, height);
        window.set_default_size(width, height);
        tracing::info!(width, height, "Prepared consistent startup window size");
    }
}

pub fn center(window: &gtk::Window, reference: Option<&gtk::Window>) {
    if !window.is_fullscreen() && !window.is_maximized() {
        platform::center(window, reference.unwrap_or(window));
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use gtk::{glib::translate::ToGlibPtr, prelude::*};
    use std::ffi::{c_char, c_void};

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Point {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Size {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Rect {
        origin: Point,
        size: Size,
    }

    unsafe extern "C" {
        fn gdk_macos_surface_get_native_window(
            surface: *mut gtk::gdk::ffi::GdkSurface,
        ) -> *mut c_void;
    }
    #[link(name = "objc")]
    #[allow(clashing_extern_declarations)]
    unsafe extern "C" {
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        fn send_id(receiver: *mut c_void, selector: *mut c_void) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        fn send_point(receiver: *mut c_void, selector: *mut c_void, point: Point);
        #[cfg(target_arch = "aarch64")]
        #[link_name = "objc_msgSend"]
        fn send_rect(receiver: *mut c_void, selector: *mut c_void) -> Rect;
        #[cfg(target_arch = "x86_64")]
        #[link_name = "objc_msgSend_stret"]
        fn send_rect_stret(result: *mut Rect, receiver: *mut c_void, selector: *mut c_void);
    }

    fn native(window: &gtk::Window) -> Option<*mut c_void> {
        let surface = window.surface()?;
        // GDK returns an NSWindow, not its contentView.
        let native = unsafe { gdk_macos_surface_get_native_window(surface.to_glib_none().0) };
        (!native.is_null()).then_some(native)
    }

    fn rect(receiver: *mut c_void, name: &std::ffi::CStr) -> Rect {
        let selector = unsafe { sel_registerName(name.as_ptr()) };
        #[cfg(target_arch = "aarch64")]
        {
            unsafe { send_rect(receiver, selector) }
        }
        #[cfg(target_arch = "x86_64")]
        {
            let mut result = Rect::default();
            unsafe { send_rect_stret(&mut result, receiver, selector) };
            result
        }
    }

    fn work_area(window: &gtk::Window) -> Option<Rect> {
        let native = native(window)?;
        let screen = unsafe { send_id(native, sel_registerName(c"screen".as_ptr())) };
        (!screen.is_null()).then(|| rect(screen, c"visibleFrame"))
    }

    pub fn work_area_size(window: &gtk::Window) -> Option<(i32, i32)> {
        let area = work_area(window)?;
        Some((area.size.width as i32, area.size.height as i32))
    }

    pub fn center(window: &gtk::Window, reference: &gtk::Window) {
        let (Some(native), Some(area)) = (native(window), work_area(reference)) else {
            return;
        };
        let frame = rect(native, c"frame");
        let (x, y) = super::centered_origin(
            area.origin.x,
            area.origin.y,
            area.size.width,
            area.size.height,
            frame.size.width,
            frame.size.height,
        );
        unsafe {
            send_point(
                native,
                sel_registerName(c"setFrameOrigin:".as_ptr()),
                Point { x, y },
            )
        };
        tracing::debug!(
            x,
            y,
            width = frame.size.width,
            height = frame.size.height,
            "Centered native window"
        );
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use gtk::prelude::*;
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }

    unsafe extern "C" {
        fn gdk_win32_surface_get_handle(surface: *mut c_void) -> *mut c_void;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MonitorFromWindow(hwnd: *mut c_void, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
        fn GetWindowRect(hwnd: *mut c_void, rect: *mut Rect) -> i32;
        fn SetWindowPos(
            hwnd: *mut c_void, after: *mut c_void, x: i32, y: i32, width: i32, height: i32,
            flags: u32,
        ) -> i32;
    }

    fn native(window: &gtk::Window) -> Option<*mut c_void> {
        let surface = window.surface()?;
        let hwnd = unsafe { gdk_win32_surface_get_handle(surface.as_ptr().cast()) };
        (!hwnd.is_null()).then_some(hwnd)
    }

    fn work_area(window: &gtk::Window) -> Option<Rect> {
        let hwnd = native(window)?;
        let mut info = MonitorInfo {
            size: std::mem::size_of::<MonitorInfo>() as u32,
            ..Default::default()
        };
        let monitor = unsafe { MonitorFromWindow(hwnd, 2) };
        (unsafe { GetMonitorInfoW(monitor, &mut info) } != 0).then_some(info.work)
    }

    pub fn work_area_size(window: &gtk::Window) -> Option<(i32, i32)> {
        let area = work_area(window)?;
        let scale = window.surface()?.scale();
        Some((
            ((area.right - area.left) as f64 / scale) as i32,
            ((area.bottom - area.top) as f64 / scale) as i32,
        ))
    }

    pub fn center(window: &gtk::Window, reference: &gtk::Window) {
        let (Some(hwnd), Some(area)) = (native(window), work_area(reference)) else {
            return;
        };
        let mut frame = Rect::default();
        if unsafe { GetWindowRect(hwnd, &mut frame) } == 0 {
            return;
        }
        // Both rectangles are Win32 coordinates. GTK alone owns DPI-aware sizing.
        let (x, y) = super::centered_origin(
            area.left as f64,
            area.top as f64,
            (area.right - area.left) as f64,
            (area.bottom - area.top) as f64,
            (frame.right - frame.left) as f64,
            (frame.bottom - frame.top) as f64,
        );
        const SWP_NOSIZE: u32 = 0x0001;
        const SWP_NOZORDER: u32 = 0x0004;
        const SWP_NOACTIVATE: u32 = 0x0010;
        let result = unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                x.round() as i32,
                y.round() as i32,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        if result == 0 {
            tracing::warn!("Unable to center the native window");
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    pub fn work_area_size(_: &gtk::Window) -> Option<(i32, i32)> {
        None
    }
    pub fn center(_: &gtk::Window, _: &gtk::Window) {
        // GTK4 delegates top-level placement to the compositor on Wayland.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_size_is_stable_on_large_displays_and_fits_small_ones() {
        assert_eq!(startup_size(1920, 1040), (1152, 720));
        assert_eq!(startup_size(3840, 2120), (1152, 720));
        assert_eq!(startup_size(1024, 680), (976, 632));
    }

    #[test]
    fn centering_respects_taskbars_negative_origins_and_native_scale() {
        assert_eq!(
            centered_origin(0.0, 0.0, 1920.0, 1040.0, 1152.0, 720.0),
            (384.0, 160.0)
        );
        assert_eq!(
            centered_origin(-1920.0, 40.0, 1920.0, 1040.0, 1152.0, 720.0),
            (-1536.0, 200.0)
        );
        assert_eq!(
            centered_origin(0.0, 0.0, 3840.0, 2080.0, 2304.0, 1440.0),
            (768.0, 320.0)
        );
    }
}
