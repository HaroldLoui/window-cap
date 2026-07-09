use windows_app::{App, Ctx, run_app, Action, Event};
use windows::Win32::UI::{
    HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext},
    WindowsAndMessaging::{
        GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOPMOST,
        WS_POPUP,
    },
};
use windows_canvas::{ColorF, DrawingSession, Rect, Result};
use windows_window::quit;

/// 应用状态 — self 就是 state
#[derive(Debug, Default)]
struct MyApp {

    fullscreen: Rect,

    start_pos: Option<(i32, i32)>,
    end_pos: Option<(i32, i32)>,
}

impl App for MyApp {
    fn update(&mut self, ctx: &Ctx, session: &DrawingSession) -> Result<bool> {
        // ── 按键语义（应用层决策，框架不强制）──
        if ctx.keys().is_down(Event::KEY_ESC) {
            quit();
            return Ok(false);
        }

        // ── 处理瞬时事件 ──
        for event in ctx.events() {
            match event {
                Action::MouseDown { button, x, y } => {
                    println!("pressed {:?} at ({}, {})", button, x, y);
                    self.start_pos = Some((*x, *y));
                }
                Action::MouseUp { button, x, y } => {
                    println!("released {:?} at ({}, {})", button, x, y);
                    self.end_pos = Some((*x, *y));
                }
                Action::MouseMove { x, y } => {
                    println!("move ({}, {})", x, y);
                }
                // Action::Resize { w, h } => {
                //     self.width = *w as f32;
                //     self.height = *h as f32;
                // }
                _ => {}
            }
        }

        let _mouse = ctx.mouse();
        // if mouse.left {
        //     println!("pressed Left at ({}, {})", mouse.x, mouse.y);
        // }

        // ── 绘制 ──
        session.clear(ColorF::TRANSPARENT);

        let brush = session.create_solid_brush(ColorF::new(0.0, 0.0, 0.0, 0.3))?;
        session.fill_rect(&self.fullscreen, &brush);

        Ok(true)
    }
}

fn main() -> Result<()> {
    let (w, h) = get_screen_size();
    run_app(
        "Overlay",
        |wb| {
            wb.style(WS_POPUP.0)
                .ex_style((WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP).0)
                .size(w, h)
        },
        |_window| {
            Ok(MyApp {
                fullscreen: Rect::from_xywh(0.0, 0.0, w as f32, h as f32),
                ..Default::default()
            })
        },
    )
}

fn get_screen_size() -> (i32, i32) {
    // DPI 感知（必须在 GetSystemMetrics 之前，框架统一处理）
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    };
    let w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    (w, h)
}
