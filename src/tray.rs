//! Close-to-tray, same idea as cursor-byok: window hide keeps MITM alive.
//! Only 托盘「退出」stops the process.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct TrayHold {
    _icon: tray_icon::TrayIcon,
    pub open_id: tray_icon::menu::MenuId,
    pub quit_id: tray_icon::menu::MenuId,
}

pub fn close_hides_to_tray(allow_exit: bool) -> bool {
    !allow_exit
}

pub fn quit_requested(allow_exit: bool) -> bool {
    allow_exit
}

/// After the first hide, `close_requested` can stay true. Re-applying
/// `Visible(false)` would swallow 托盘「打开」. Tray「退出」must not hide.
pub fn should_apply_hide(allow_exit: bool, already_hidden: bool) -> bool {
    close_hides_to_tray(allow_exit) && !already_hidden
}

#[cfg(windows)]
mod win32 {
    #[link(name = "user32")]
    extern "system" {
        pub fn FindWindowW(class: *const u16, title: *const u16) -> isize;
        pub fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
        pub fn SetForegroundWindow(hwnd: isize) -> i32;
        pub fn BringWindowToTop(hwnd: isize) -> i32;
    }
    pub const SW_SHOW: i32 = 5;
    pub const SW_RESTORE: i32 = 9;
}

pub fn show_native_window() {
    #[cfg(windows)]
    unsafe {
        let title: Vec<u16> = "Grok-Bot-Auth\0".encode_utf16().collect();
        let hwnd = win32::FindWindowW(std::ptr::null(), title.as_ptr());
        if hwnd != 0 {
            win32::ShowWindow(hwnd, win32::SW_RESTORE);
            win32::ShowWindow(hwnd, win32::SW_SHOW);
            win32::BringWindowToTop(hwnd);
            win32::SetForegroundWindow(hwnd);
        }
    }
}

fn icon_32() -> Option<tray_icon::Icon> {
    let src = include_bytes!("../assets/icon.rgba");
    const SRC: u32 = 256;
    const DST: u32 = 32;
    if src.len() != (SRC * SRC * 4) as usize {
        return None;
    }
    let mut out = vec![0u8; (DST * DST * 4) as usize];
    for y in 0..DST {
        for x in 0..DST {
            let sx = x * SRC / DST;
            let sy = y * SRC / DST;
            let si = ((sy * SRC + sx) * 4) as usize;
            let di = ((y * DST + x) * 4) as usize;
            out[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    tray_icon::Icon::from_rgba(out, DST, DST).ok()
}

fn park_and_exit(rt: &tokio::runtime::Handle, state: &crate::api::AppState) -> ! {
    if let Err(error) = rt.block_on(crate::api::park_cursor_mitm(state)) {
        eprintln!("park Cursor 反代 failed: {error}");
    }
    std::process::exit(0);
}

pub fn install(
    allow_exit: Arc<AtomicBool>,
    show_window: Arc<AtomicBool>,
    ctx: egui::Context,
    rt: tokio::runtime::Handle,
    state: crate::api::AppState,
) -> Option<TrayHold> {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let menu = Menu::new();
    let open = MenuItem::new("打开 Grok-Bot-Auth", true, None);
    let quit = MenuItem::new("退出", true, None);
    menu.append(&open).ok()?;
    menu.append(&quit).ok()?;
    let icon = icon_32()?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Grok-Bot-Auth · 后台运行中")
        .with_icon(icon)
        .build()
        .ok()?;
    let hold = TrayHold {
        _icon: tray,
        open_id: open.id().clone(),
        quit_id: quit.id().clone(),
    };
    let open_id = hold.open_id.clone();
    let quit_id = hold.quit_id.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let mut wake = false;
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window.store(true, Ordering::SeqCst);
                show_native_window();
                wake = true;
            }
        }
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == quit_id {
                allow_exit.store(true, Ordering::SeqCst);
                ctx.request_repaint();
                park_and_exit(&rt, &state);
            }
            if event.id == open_id {
                show_window.store(true, Ordering::SeqCst);
                show_native_window();
                wake = true;
            }
        }
        if wake {
            ctx.request_repaint();
        }
    });
    Some(hold)
}

#[allow(dead_code)]
pub fn poll(hold: &TrayHold, allow_exit: &AtomicBool) -> TrayAction {
    use tray_icon::menu::MenuEvent;
    use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            return TrayAction::Show;
        }
    }
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        if event.id == hold.quit_id {
            allow_exit.store(true, Ordering::SeqCst);
            return TrayAction::Quit;
        }
        if event.id == hold.open_id {
            return TrayAction::Show;
        }
    }
    TrayAction::None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    None,
    Show,
    Quit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_close_hides_unless_tray_quit() {
        assert!(close_hides_to_tray(false));
        assert!(!close_hides_to_tray(true));
        assert!(should_apply_hide(false, false));
        assert!(!should_apply_hide(false, true));
        assert!(!should_apply_hide(true, false));
        assert!(!should_apply_hide(true, true), "sticky close after hide must not eat 退出");
        assert!(quit_requested(true));
        assert!(!quit_requested(false));
    }

    #[test]
    fn tray_icon_bytes_are_256() {
        assert_eq!(include_bytes!("../assets/icon.rgba").len(), 256 * 256 * 4);
    }
}
