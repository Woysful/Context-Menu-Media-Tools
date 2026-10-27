//! Transparent window and blur behind it. Both are optional: where the system does not
//! support them (or the settings say no) the window is simply opaque.
//!
//!  * Windows 11: the acrylic backdrop of the window manager (DWM).
//!  * KDE Plasma on Wayland: the blur protocol of KWin.
//!  * everywhere else: no blur. A transparent window is only used where a compositor is
//!    certain (Wayland, Windows); on X11 it has to be switched on by hand.

use crate::settings::Settings;

fn env_is(var: &str, value: &str) -> bool {
    std::env::var(var).is_ok_and(|v| v.eq_ignore_ascii_case(value))
}

pub fn is_wayland() -> bool {
    env_is("XDG_SESSION_TYPE", "wayland") || std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
}

fn is_kde() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase().contains("kde")
}

/// Should the window be created transparent?
pub fn transparency_enabled(s: &Settings) -> bool {
    match s.transparency.as_str() {
        "on" => true,
        "off" => false,
        _ => cfg!(windows) || (cfg!(unix) && is_wayland()),
    }
}

/// Can the system blur what is behind the window?
pub fn blur_supported() -> bool {
    cfg!(windows) || (cfg!(all(unix, not(target_os = "macos"))) && is_wayland() && is_kde())
}

/// Switch the blur behind the window on or off. Errors are returned, never raised.
pub fn set_blur(frame: &eframe::Frame, enable: bool, dark: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        return windows_blur(frame, enable, dark);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = dark;
        return wl::set(frame, enable);
    }
    #[allow(unreachable_code)]
    {
        let _ = (frame, enable, dark);
        Err("not supported here".into())
    }
}

#[cfg(windows)]
fn windows_blur(frame: &eframe::Frame, enable: bool, dark: bool) -> Result<(), String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmSetWindowAttribute(hwnd: isize, attribute: u32, value: *const core::ffi::c_void, size: u32) -> i32;
    }
    let handle = frame.window_handle().map_err(|e| e.to_string())?;
    let RawWindowHandle::Win32(w) = handle.as_raw() else { return Err("not a Win32 window".into()) };
    let hwnd = w.hwnd.get();
    let dark_mode: i32 = if dark { 1 } else { 0 };
    // DWMSBT_TRANSIENTWINDOW (acrylic) = 3, DWMSBT_NONE = 1
    let backdrop: i32 = if enable { 3 } else { 1 };
    unsafe {
        // 20 = DWMWA_USE_IMMERSIVE_DARK_MODE, 38 = DWMWA_SYSTEMBACKDROP_TYPE (Windows 11 22H2 and later)
        DwmSetWindowAttribute(hwnd, 20, &dark_mode as *const i32 as *const core::ffi::c_void, 4);
        let r = DwmSetWindowAttribute(hwnd, 38, &backdrop as *const i32 as *const core::ffi::c_void, 4);
        if r != 0 {
            return Err(format!("DwmSetWindowAttribute returned {r}"));
        }
    }
    Ok(())
}

/// KDE Plasma on Wayland: the blur protocol of KWin, spoken over the connection of the window.
#[cfg(all(unix, not(target_os = "macos")))]
mod wl {
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
    use std::sync::Mutex;
    use wayland_backend::client::{Backend, ObjectId};
    use wayland_client::globals::{registry_queue_init, GlobalListContents};
    use wayland_client::protocol::{wl_registry::WlRegistry, wl_surface::WlSurface};
    use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
    use wayland_protocols_plasma::blur::client::{org_kde_kwin_blur::OrgKdeKwinBlur, org_kde_kwin_blur_manager::OrgKdeKwinBlurManager};

    pub struct State;
    impl Dispatch<WlRegistry, GlobalListContents> for State {
        fn event(_: &mut Self, _: &WlRegistry, _: wayland_client::protocol::wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {
        }
    }
    impl Dispatch<OrgKdeKwinBlurManager, ()> for State {
        fn event(_: &mut Self, _: &OrgKdeKwinBlurManager, _: <OrgKdeKwinBlurManager as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
    }
    impl Dispatch<OrgKdeKwinBlur, ()> for State {
        fn event(_: &mut Self, _: &OrgKdeKwinBlur, _: <OrgKdeKwinBlur as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
    }

    struct Blur {
        conn: Connection,
        qh: QueueHandle<State>,
        _queue: EventQueue<State>,
        manager: OrgKdeKwinBlurManager,
        surface: WlSurface,
        blur: Option<OrgKdeKwinBlur>,
    }

    static STATE: Mutex<Option<Blur>> = Mutex::new(None);

    fn init(frame: &eframe::Frame) -> Result<Blur, String> {
        let window = frame.window_handle().map_err(|e| e.to_string())?.as_raw();
        let display = frame.display_handle().map_err(|e| e.to_string())?.as_raw();
        let (RawWindowHandle::Wayland(w), RawDisplayHandle::Wayland(d)) = (window, display) else {
            return Err("not a Wayland window".into());
        };
        unsafe {
            let backend = Backend::from_foreign_display(d.display.as_ptr().cast());
            let conn = Connection::from_backend(backend);
            let (globals, mut queue) = registry_queue_init::<State>(&conn).map_err(|e| e.to_string())?;
            let qh = queue.handle();
            let manager: OrgKdeKwinBlurManager = globals.bind(&qh, 1..=1, ()).map_err(|_| "the compositor has no blur protocol".to_string())?;
            let id = ObjectId::from_ptr(WlSurface::interface(), w.surface.as_ptr().cast()).map_err(|e| e.to_string())?;
            let surface = WlSurface::from_id(&conn, id).map_err(|e| e.to_string())?;
            queue.roundtrip(&mut State).map_err(|e| e.to_string())?;
            Ok(Blur { conn, qh, _queue: queue, manager, surface, blur: None })
        }
    }

    pub fn set(frame: &eframe::Frame, enable: bool) -> Result<(), String> {
        let mut guard = STATE.lock().map_err(|_| "blur state is poisoned".to_string())?;
        if guard.is_none() {
            if !enable {
                return Ok(());
            }
            *guard = Some(init(frame)?);
        }
        let st = guard.as_mut().expect("just set");
        if enable {
            if st.blur.is_none() {
                let b = st.manager.create(&st.surface, &st.qh, ());
                b.set_region(None); // the whole window
                b.commit();
                st.blur = Some(b);
            }
        } else if let Some(b) = st.blur.take() {
            st.manager.unset(&st.surface);
            b.release();
        }
        st.conn.flush().map_err(|e| e.to_string())
    }
}
