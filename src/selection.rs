use windows_app::{Action, MouseButton, Pos2};
use windows_canvas::{Brush, ColorF, DrawingSession, Rect, Result};

use crate::{brush::BrushState, utils::normalize};

// ── 常量 ─────────────────────────────────────────────────────────────

/// 默认 overlay 半透明颜色
pub const OVERLAY_COLOR: ColorF = ColorF::new(0.0, 0.0, 0.0, 0.3);

// ── Selection ────────────────────────────────────────────────────────

/// 矩形挖空选区 — 管理屏幕上的选区以及 overlay 绘制
///
/// # 职责
/// - 通过鼠标拖拽创建矩形选区
/// - 绘制半透明 overlay，在选区位置"挖空"（露出透明背景）
///
/// # 未来扩展
/// - `resize(dw, dh)` — 调整选区大小（从handles）
/// - `move_by(dx, dy)` — 移动选区位置
/// - `aspect_ratio_lock` — 锁定宽高比
/// - edge/ corner handles 拖拽调整
pub struct Selection {
    /// 全屏边界（绘制 overlay 时确定填充范围）
    fullscreen: Rect,

    /// 选区起点（鼠标按下位置）
    start_pos: Option<Pos2>,
    /// 选区终点（鼠标抬起位置）
    end_pos: Option<Pos2>,
    /// 是否进入拖拽
    is_dragging: bool,
    /// 遮罩层 Brush
    overlay_brush: BrushState,
    /// 选区边框 Brush
    border_brush: BrushState,
}

impl Selection {
    /// 创建一个新的选区工具
    pub fn new(fullscreen: Rect) -> Self {
        Self {
            fullscreen,
            start_pos: None,
            end_pos: None,
            is_dragging: false,
            overlay_brush: BrushState::new(OVERLAY_COLOR, 0.0),
            border_brush: BrushState::new(ColorF::RED, 5.0),
        }
    }

    // ── 事件处理 ────────────────────────────────────────────────────

    /// 处理输入事件（鼠标按下/抬起）
    ///
    /// 返回 `true` 表示该事件被选区消费（外部可据此决定是否继续传递给其它工具）
    pub fn handle_event(&mut self, events: &[Action]) {
        for event in events {
            match *event {
                Action::MouseDown { button: MouseButton::Left, pos } => {
                    self.start_pos = Some(pos);
                    self.end_pos = None;
                    self.is_dragging = true;
                }
                Action::MouseUp { button: MouseButton::Left, pos } => {
                    self.end_pos = Some(pos);
                    self.is_dragging = false;
                }
                Action::MouseMove { pos } => {
                    if self.is_dragging {
                        self.end_pos = Some(pos);
                    }
                }
                _ => {},
            }
        }
    }

    // ── 查询（预留接口，后续扩展启用）───────────────────────────────

    /// 获取归一化的选区矩形（确保 left ≤ right, top ≤ bottom）
    ///
    /// 返回 `None` 表示选区不完整。
    pub fn bounds(&self) -> Option<Rect> {
        let start = self.start_pos?;
        let end = self.end_pos?;
        if end.x - start.x <= 5.0 || end.y - start.y <= 5.0 {
            return None;
        }
        Some(normalize(start, end))
    }

    /// 获取归一化的四个边值（left, top, right, bottom）
    #[allow(dead_code)]
    pub fn edges(&self) -> Option<(f32, f32, f32, f32)> {
        let r = self.bounds()?;
        Some((r.left, r.top, r.right, r.bottom))
    }

    // ── 变更（预留接口，后续扩展启用）───────────────────────────────

    /// 重置选区（清空起点和终点）
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.start_pos = None;
        self.end_pos = None;
    }

    // ── 绘制 ───────────────────────────────────────────────────────

    /// 绘制半透明 overlay，在选区位置挖空
    pub fn draw_overlay(&mut self, session: &DrawingSession) -> Result<()> {
        let cutout = self.bounds();
        let overlay = self.overlay_brush.brush(session)?;
        
        if let Some(rect) = cutout { 
            // 画 挖空区域
            let start = self.start_pos.unwrap();
            let end = self.end_pos.unwrap();
            draw_cutout(session, overlay, start, end, &self.fullscreen);

            // 画 挖空区域 边框
            let width = self.border_brush.stroke_width;
            let border = self.border_brush.brush(session)?;
            session.draw_rect(&rect, border, width);
        } else {
            // 全屏遮罩层
            session.fill_rect(&self.fullscreen, overlay);
        }

        Ok(())
    }

    // ── 内部辅助 ────────────────────────────────────────────────────
}

// ── 未来扩展预留 ────────────────────────────────────────────────────
//
// 以下方法为未来功能预留接口，当实现 调整大小 / 移动 等功能时启用：
//
// impl Selection {
//     /// 按偏移量移动选区
//     pub fn move_by(&mut self, dx: f32, dy: f32) {
//         if let (Some(ref mut start), Some(ref mut end)) = (self.start_pos.as_mut(), self.end_pos.as_mut()) {
//             start.x += dx;
//             start.y += dy;
//             end.x += dx;
//             end.y += dy;
//         }
//     }
//
//     /// 从右下角调整选区大小（delta 像素）
//     pub fn resize(&mut self, dw: f32, dh: f32) {
//         if let Some(ref mut end) = self.end_pos {
//             end.x = (end.x + dw).max(self.start_pos.map_or(0.0, |s| s.x));
//             end.y = (end.y + dh).max(self.start_pos.map_or(0.0, |s| s.y));
//         }
//     }
//
//     /// 设置选区的绝对位置（保持宽高不变）
//     pub fn move_to(&mut self, x: f32, y: f32) {
//         if let (Some(start), Some(end)) = (self.start_pos, self.end_pos) {
//             let w = end.x - start.x;
//             let h = end.y - start.y;
//             self.start_pos = Some(Pos2::new(x, y));
//             self.end_pos = Some(Pos2::new(x + w, y + h));
//         }
//     }
// }


/// 绘制"挖空"效果：在 fullscreen 上画 4 个矩形，留出选区区域
fn draw_cutout(
    session: &DrawingSession,
    brush: &Brush,
    start: Pos2,
    end: Pos2,
    fullscreen: &Rect,
) {
    let (left, right) = if start.x <= end.x { (start.x, end.x) } else { (end.x, start.x) };
    let (top, bottom) = if start.y <= end.y { (start.y, end.y) } else { (end.y, start.y) };

    // 左边：x=0 ~ left, 高度 = 全屏高
    let r = Rect::new(0.0, 0.0, left, fullscreen.bottom);
    session.fill_rect(&r, brush);
    // 上边：x=left ~ right, 高度 = top
    let r = Rect::new(left, 0.0, right, top);
    session.fill_rect(&r, brush);
    // 右边：x=right ~ 全屏右, 高度 = 全屏高
    let r = Rect::new(right, 0.0, fullscreen.right , fullscreen.bottom);
    session.fill_rect(&r, brush);
    // 下边：x=left ~ right, y=bottom ~ 全屏底
    let r = Rect::new(left, bottom, right, fullscreen.bottom);
    session.fill_rect(&r, brush);
}