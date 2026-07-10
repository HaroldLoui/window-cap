use windows_app::{Action, MouseButton, Pos2};
use windows_canvas::{ColorF, DrawingSession, Rect, Result};

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
/// - `resize(dw, dh)` — 调整选区大小（从右下角或 edge handles）
/// - `move_by(dx, dy)` — 移动选区位置
/// - `aspect_ratio_lock` — 锁定宽高比
/// - edge/ corner handles 拖拽调整
#[derive(Debug, Clone)]
pub struct Selection {
    /// 全屏边界（绘制 overlay 时确定填充范围）
    fullscreen: Rect,

    /// 选区起点（鼠标按下位置）
    start_pos: Option<Pos2>,
    /// 选区终点（鼠标抬起位置）
    end_pos: Option<Pos2>,

    /// overlay 半透明颜色
    overlay_color: ColorF,

    is_dragging: bool,
}

impl Selection {
    /// 创建一个新的选区工具
    pub fn new(fullscreen: Rect) -> Self {
        Self {
            fullscreen,
            start_pos: None,
            end_pos: None,
            overlay_color: OVERLAY_COLOR,
            is_dragging: false,
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

    /// 是否有完整的选区（起点和终点都已确定）
    #[allow(dead_code)]
    pub fn has_selection(&self) -> bool {
        self.start_pos.is_some() && self.end_pos.is_some()
    }

    /// 获取归一化的选区矩形（确保 left ≤ right, top ≤ bottom）
    ///
    /// 返回 `None` 表示选区不完整。
    #[allow(dead_code)]
    pub fn bounds(&self) -> Option<Rect> {
        let start = self.start_pos?;
        let end = self.end_pos?;
        Some(Self::normalize(start, end))
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

    /// 设置 overlay 颜色
    #[allow(dead_code)]
    pub fn set_overlay_color(&mut self, color: ColorF) {
        self.overlay_color = color;
    }

    /// 更新全屏边界（窗口大小变化时调用）
    #[allow(dead_code)]
    pub fn set_fullscreen(&mut self, fullscreen: Rect) {
        self.fullscreen = fullscreen;
    }

    // ── 绘制 ───────────────────────────────────────────────────────

    /// 绘制半透明 overlay，在选区位置挖空
    pub fn draw_overlay(&mut self, session: &DrawingSession) -> Result<()> {
        let brush = session.create_solid_brush(self.overlay_color)?;

        if let (Some(start), Some(end)) = (self.start_pos, self.end_pos) {
            Self::draw_cutout(session, &brush, start, end, &self.fullscreen);
        } else {
            // 没有选区 → 全屏覆盖
            session.fill_rect(&self.fullscreen, &brush);
        }

        Ok(())
    }

    // ── 内部辅助 ────────────────────────────────────────────────────

    /// 绘制"挖空"效果：在 fullscreen 上画 4 个矩形，留出选区区域
    fn draw_cutout(
        session: &DrawingSession,
        brush: &windows_canvas::Brush,
        start: Pos2,
        end: Pos2,
        fullscreen: &Rect,
    ) {
        let (left, right) = if start.x <= end.x { (start.x, end.x) } else { (end.x, start.x) };
        let (top, bottom) = if start.y <= end.y { (start.y, end.y) } else { (end.y, start.y) };

        // 左边：x=0 ~ left, 高度 = 全屏高
        let r = Rect::from_xywh(0.0, 0.0, left, fullscreen.bottom);
        session.fill_rect(&r, brush);
        // 上边：x=left ~ right, 高度 = top
        let r = Rect::from_xywh(left, 0.0, right - left, top);
        session.fill_rect(&r, brush);
        // 右边：x=right ~ 全屏右, 高度 = 全屏高
        let r = Rect::from_xywh(right, 0.0, fullscreen.right - right, fullscreen.bottom);
        session.fill_rect(&r, brush);
        // 下边：x=left ~ right, y=bottom ~ 全屏底
        let r = Rect::from_xywh(left, bottom, right - left, fullscreen.bottom - bottom);
        session.fill_rect(&r, brush);
    }

    /// 将两个点归一化为标准矩形
    #[allow(dead_code)]
    fn normalize(a: Pos2, b: Pos2) -> Rect {
        let left = a.x.min(b.x);
        let right = a.x.max(b.x);
        let top = a.y.min(b.y);
        let bottom = a.y.max(b.y);
        // Rect::from_xywh(x, y, width, height)
        Rect::from_xywh(left, top, right - left, bottom - top)
    }
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