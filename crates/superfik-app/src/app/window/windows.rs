//! Keep the initial native frame inside its monitor's work area, in pixels.

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

pub fn fit_monitor(window: &Window) {
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };
    let Ok(handle) = window.window_handle() else { return };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else { return };
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: the HWND is owned by this window; the monitor handle is borrowed
    #[allow(unsafe_code)]
    let available = unsafe {
        let monitor = MonitorFromWindow(handle.hwnd.get() as _, MONITOR_DEFAULTTONEAREST);
        GetMonitorInfoW(monitor, &mut info) != 0
    };
    if !available {
        return;
    }
    let work = info.rcWork;
    let width = work.right.saturating_sub(work.left) as u32;
    let height = work.bottom.saturating_sub(work.top) as u32;
    let outer = window.outer_size();
    let inner = window.inner_size();
    let border = PhysicalSize::new(outer.width.saturating_sub(inner.width), outer.height.saturating_sub(inner.height));
    let room = PhysicalSize::new(width.saturating_sub(border.width), height.saturating_sub(border.height));
    if room.width == 0 || room.height == 0 {
        return;
    }
    let minimum = winit::dpi::LogicalSize::new(980.0, 640.0).to_physical::<u32>(window.scale_factor());
    window.set_min_inner_size(Some(PhysicalSize::new(minimum.width.min(room.width), minimum.height.min(room.height))));
    let size = PhysicalSize::new(inner.width.min(room.width), inner.height.min(room.height));
    let _ = window.request_inner_size(size);
    let frame_width = size.width.saturating_add(border.width);
    let frame_height = size.height.saturating_add(border.height);
    window.set_outer_position(PhysicalPosition::new(
        work.left + width.saturating_sub(frame_width) as i32 / 2,
        work.top + height.saturating_sub(frame_height) as i32 / 2,
    ));
}
