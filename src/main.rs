mod event;

use event::{Action, Event};
use windows::Win32::{
    Foundation::HWND, Graphics::DirectComposition::{DCompositionCreateDevice2, IDCompositionDesktopDevice}, UI::{HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext}, WindowsAndMessaging::{
        GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN, WS_EX_NOREDIRECTIONBITMAP,
        WS_EX_TOPMOST, WS_POPUP,
    }},
};
use windows_canvas::*;
use windows_window::*;

use crate::event::MouseButton;

fn main() -> Result<()> {
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)?; };
    
    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };

    let window = Window::new("Overlay")
        .style(WS_POPUP.0)
        .ex_style((WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP).0) 
        .on_message(|hwnd, msg, wparam, lparam| {
            handler(&Event::from_raw(hwnd, msg, wparam, lparam))
        })
        .size(screen_w, screen_h)
        .create()?;

    let device = GpuDevice::new()?;
    let (width, height) = window.client_size();
    let mut chain = device.create_swap_chain(width as u32, height as u32)?;

    let dcomp: IDCompositionDesktopDevice = unsafe { 
        DCompositionCreateDevice2(device.d2d_device())? 
    };
    let target = unsafe { dcomp.CreateTargetForHwnd(HWND(window.hwnd()), true)? };
    let visual = unsafe { dcomp.CreateVisual()? };
    unsafe {
        visual.SetContent(chain.raw_swap_chain())?;
        target.SetRoot(&visual)?;
        dcomp.Commit()?;
    }

    let (_dcomp, _target, _visual) = (dcomp, target, visual); // keep alive

    run_with(|| {
        let session = chain.begin_draw()?;
        session.clear(ColorF::TRANSPARENT);

        // draw overlay
        let brush = session.create_solid_brush(ColorF::new(0.0, 0.0, 0.0, 0.3))?;
        let full_rect = Rect::new(0.0, 0.0, width as f32, height as f32);
        session.fill_rect(&full_rect, &brush);

        drop(session);
        chain.present()?;
        Ok(true)
    })
}

fn handler(event: &Event) -> Option<isize> {
    match &event.action {
        Action::KeyDown { key } if *key == Event::KEY_ESC => {
            quit();
            Some(0)
        }
        Action::MouseDown { button, x, y } if *button == MouseButton::Left => {
            println!("mouse: ({}, {})", x, y);
            None
        }
        Action::MouseUp { button, x, y } if *button == MouseButton::Left => {
            println!("mouse: ({}, {})", x, y);
            None
        }
        _ => None,
    }
}
