use windows::Win32::UI::{
    HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext},
    WindowsAndMessaging::{
        GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOPMOST,
        WS_POPUP,
    },
};
use windows_app::{App, Ctx, Key, run_app};
use windows_canvas::{ColorF, DrawingSession, Rect, Result};
use windows_window::quit;

mod brush;
mod selection;
mod utils;

use selection::Selection;

/// 应用状态 — self 就是 state
struct MyApp {
    /// 挖空选区工具 — 管理 overlay 绘制和选区交互
    selection: Selection,
}

impl App for MyApp {
    fn update(&mut self, ctx: &Ctx, session: &DrawingSession) -> Result<bool> {
        // ── 按键语义（应用层决策，框架不强制）──
        if ctx.keys().is_down(Key::Escape) {
            quit();
            return Ok(false);
        }

        // ── 将事件分发给选区工具 ──
        self.selection.handle_event(ctx.events());
        selection::handles::set_cursor(self.selection.cursor_style());

        // ── 绘制 ──
        session.clear(ColorF::TRANSPARENT);
        self.selection.draw(session)?;

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
                selection: Selection::new(Rect::from_xywh(0.0, 0.0, w as f32, h as f32)),
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
