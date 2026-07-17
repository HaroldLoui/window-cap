use windows::winuser::{
    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOPMOST, WS_POPUP,
};
use windows_canvas::{ColorF, DrawingSession, Result};
use windows_cap_core::{App, Ctx, run_app};

mod app;
mod brush;
mod capture;
mod selection;

use app::Screenshot;

impl App for Screenshot {
    fn update(&mut self, ctx: &Ctx, session: &DrawingSession) -> Result<bool> {
        // ── 惰性创建 GPU bitmap ──
        self.ensure_bitmap(session)?;

        // ── 事件分发 ──
        self.selection.handle_event(ctx.events());
        selection::handles::set_cursor(self.selection.cursor_style());

        // ── 绘制：底图 + 挖空遮罩 ──
        session.clear(ColorF::TRANSPARENT);
        self.draw_background(session);
        self.selection.draw_overlay_only(session)?;

        // ── 按键处理（在绘制之后，边框之前）──
        let cont = self.handle_keys(ctx.keys(), session)?;
        if !cont {
            return Ok(false); // handle_keys 内部会调用 save_region + quit
        }

        // ── 绘制：边框 + 手柄 ──
        self.selection.draw_border_and_handles(session)?;

        Ok(true)
    }
}

fn main() -> Result<()> {
    let (w, h) = app::get_screen_size();

    // 预截全屏（overlay 窗口创建前，截图是纯净桌面）
    let (pixels, w, h) = capture::capture_screen(w, h)?;

    run_app(
        "Overlay",
        |wb| {
            wb.style(WS_POPUP)
                .ex_style(WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP)
                .size(w, h)
        },
        |_window| Ok(Screenshot::new(pixels, w, h)),
    )
}

