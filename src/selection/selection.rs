use windows_app::{Action, MouseButton, Pos2};
use windows_canvas::{Brush, ColorF, DrawingSession, Rect, Result};

use crate::{
    brush::BrushState,
    selection::handles::{CursorStyle, Handle, calc_handles},
    utils::normalize,
};

// ── 常量 ─────────────────────────────────────────────────────────────

/// 默认 overlay 半透明颜色
const OVERLAY_COLOR: ColorF = ColorF::new(0.0, 0.0, 0.0, 0.3);
/// 最小选区大小
const MIN_SELECTION_SIZE: f32 = 5.0;

// ── Selection ────────────────────────────────────────────────────────

/// 矩形挖空选区 — 管理屏幕上的选区以及 overlay 绘制
///
/// # 职责
/// - 通过鼠标拖拽创建矩形选区
/// - 绘制半透明 overlay，在选区位置"挖空"（露出透明背景）
pub struct Selection {
    /// 全屏边界（绘制 overlay 时确定填充范围）
    fullscreen: Rect,
    /// 选区起点（鼠标按下位置）
    start_pos: Option<Pos2>,
    /// 选区终点（鼠标抬起位置）
    end_pos: Option<Pos2>,
    /// 选区移动起始点
    drag_origin: Option<Pos2>,
    /// 当前选中的手柄
    active_handle: Option<Handle>,
    /// 是否hover了某个手柄
    hover_handle: Option<Handle>,
    /// 当前状态
    state: State,
    /// 遮罩层 Brush
    overlay_brush: BrushState,
    /// 选区边框 Brush
    border_brush: BrushState,
    /// 白色边框 Brush
    hover_handle_brush: BrushState,
}

impl Selection {
    /// 创建一个新的选区工具
    pub fn new(fullscreen: Rect) -> Self {
        Self {
            fullscreen,
            start_pos: None,
            end_pos: None,
            drag_origin: None,
            active_handle: None,
            hover_handle: None,
            state: State::None,
            overlay_brush: BrushState::new(OVERLAY_COLOR, 0.0),
            border_brush: BrushState::new(ColorF::RED, 4.0),
            hover_handle_brush: BrushState::new(ColorF::WHITE, 1.0),
        }
    }

    // ── 事件处理 ────────────────────────────────────────────────────

    /// 处理输入事件（鼠标按下/抬起/移动）
    ///
    /// 状态机转换：
    /// - None    + MouseDown    → Selecting  （开始框选）
    /// - Selecting + MouseMove  → Selecting  （更新 end_pos）
    /// - Selecting + MouseUp    → Idle / None （选区有效 / 太小则丢弃）
    /// - Idle    + MouseDown in → Move       （拖拽移动选区）
    /// - Idle    + MouseDown on handle → Resize
    /// - Idle    + MouseDown out → Selecting （取消旧选区，重新框选）
    /// - Move    + MouseUp      → Idle
    /// - Resize  + MouseUp      → Idle
    pub fn handle_event(&mut self, events: &[Action]) {
        for event in events {
            match *event {
                Action::MouseDown {
                    button: MouseButton::Left,
                    pos,
                } => {
                    match self.state {
                        State::None => {
                            // 全屏遮罩 → 开始框选
                            self.start_pos = Some(pos);
                            self.end_pos = None;
                            self.state = State::Selecting;
                        }
                        State::Idle => {
                            if let Some(handle) = self.hit_handle_fn(pos) {
                                self.drag_origin = Some(pos);
                                self.active_handle = Some(handle);
                                self.state = State::Resize;
                            } else if self.in_selection(pos) {
                                self.drag_origin = Some(pos);
                                self.state = State::Move;
                            } else {
                                // 点在选区外 → 取消旧选区，重新框选
                                self.start_pos = Some(pos);
                                self.end_pos = None;
                                self.state = State::Selecting;
                            }
                        }
                        _ => {}
                    }
                }
                Action::MouseUp {
                    button: MouseButton::Left,
                    pos,
                } => {
                    match self.state {
                        State::Selecting => {
                            self.end_pos = Some(pos);
                            // 选区太小（≤ 5px）则丢弃，回到 None
                            if let Some(rect) = self.bounds() {
                                // 归一化：确保 start = 左上，end = 右下
                                self.start_pos = Some(Pos2::new(rect.left, rect.top));
                                self.end_pos = Some(Pos2::new(rect.right, rect.bottom));
                                self.state = State::Idle;
                            } else {
                                self.start_pos = None;
                                self.end_pos = None;
                                self.state = State::None;
                            }
                        }
                        State::Move | State::Resize => {
                            self.drag_origin = None;
                            self.active_handle = None;
                            self.state = State::Idle;
                        }
                        _ => {}
                    }
                }
                Action::MouseMove { pos } => match self.state {
                    State::Selecting => {
                        self.end_pos = Some(pos);
                    }
                    State::Move => {
                        if let Some(origin) = self.drag_origin {
                            let dx = pos.x - origin.x;
                            let dy = pos.y - origin.y;
                            // 平移选区
                            self.move_by(dx, dy);
                            self.drag_origin = Some(pos); // 更新基准点
                        }
                    }
                    State::Resize => {
                        if let (Some(origin), Some(handle)) =
                            (self.drag_origin, self.active_handle)
                        {
                            let dx = pos.x - origin.x;
                            let dy = pos.y - origin.y;
                            self.resize_by(handle, dx, dy);
                            if let Some(new_h) = self.fix_flip(handle) {
                                self.active_handle = Some(new_h);
                                self.hover_handle = Some(new_h);
                            }
                            self.drag_origin = Some(pos);
                        }
                    }
                    State::Idle => {
                        self.hover_handle = self.hit_handle_fn(pos);
                    }
                    _ => {}
                },
                _ => {}
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
        if (end.x - start.x).abs() <= MIN_SELECTION_SIZE || (end.y - start.y).abs() <= MIN_SELECTION_SIZE {
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

    /// 当前位置是否在选区内
    fn in_selection(&self, pos: Pos2) -> bool {
        let Some(rect) = self.bounds() else {
            return false;
        };

        pos.x >= rect.left && pos.x <= rect.right && pos.y >= rect.top && pos.y <= rect.bottom
    }

    /// 是否命中手柄
    fn hit_handle_fn(&self, pos: Pos2) -> Option<Handle> {
        let rect = self.bounds()?;
        let handles = calc_handles(rect.left, rect.top, rect.width(), rect.height());
        for h in handles {
            if pos.x >= h.rect.left
                && pos.x < h.rect.right
                && pos.y >= h.rect.top
                && pos.y < h.rect.bottom
            {
                return Some(h.handle);
            }
        }
        None
    }

    // ── 变更（预留接口，后续扩展启用）───────────────────────────────

    /// 重置选区（清空起点和终点）
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.start_pos = None;
        self.end_pos = None;
    }

    /// 绘制 选区 相关元素
    pub fn draw(&mut self, session: &DrawingSession) -> Result<()> {
        let cutout = self.bounds();
        let overlay = self.overlay_brush.brush(session)?;

        if let Some(rect) = cutout {
            // 画 挖空区域
            draw_cutout(session, overlay, &rect, &self.fullscreen);

            // 画 挖空区域 边框
            let border_width = self.border_brush.stroke_width;
            let border = self.border_brush.brush(session)?;
            session.draw_rect(&rect, border, border_width);

            // 画 手柄
            let handles = calc_handles(rect.left, rect.top, rect.width(), rect.height());
            let hover_width = self.hover_handle_brush.stroke_width;
            let hover_brush = self.hover_handle_brush.brush(session)?;

            for h in &handles {
                // 普通手柄：填充
                h.draw(session, border);
                if self.hover_handle.is_some_and(|hh| hh == h.handle) {
                    // 悬停的手柄：画白色边框
                    session.draw_rect(&h.rect, hover_brush, hover_width);
                }
            }
        } else {
            // 全屏遮罩层
            session.fill_rect(&self.fullscreen, overlay);
        }

        Ok(())
    }

    // ── 内部辅助 ────────────────────────────────────────────────────

    // 获取鼠标样式
    pub fn cursor_style(&self) -> CursorStyle {
        match self.state {
            State::None | State::Selecting => CursorStyle::Cross,
            State::Move => CursorStyle::SizeAll,
            State::Resize => match self.active_handle {
                Some(handle) => handle.get_cursor_style(),
                _ => CursorStyle::Arrow,
            },
            State::Idle => {
                if let Some(handle) = self.hover_handle {
                    // 悬停在手柄上 → 对应的 resize 光标
                    handle.get_cursor_style()
                } else {
                    // 没有选区时 Arrow，有选区时看是否在选区内
                    CursorStyle::Arrow
                }
            }
        }
    }

    /// 按偏移量移动选区（不超出屏幕边界）
    fn move_by(&mut self, dx: f32, dy: f32) {
        if let (Some(ref mut start), Some(ref mut end)) =
            (self.start_pos.as_mut(), self.end_pos.as_mut())
        {
            start.x += dx;
            start.y += dy;
            end.x += dx;
            end.y += dy;

            // clamp 到屏幕内
            let rect = normalize(Pos2::new(start.x, start.y), Pos2::new(end.x, end.y));
            let mut shift_x = 0.0;
            let mut shift_y = 0.0;

            if rect.left < self.fullscreen.left {
                shift_x = self.fullscreen.left - rect.left;
            } else if rect.right > self.fullscreen.right {
                shift_x = self.fullscreen.right - rect.right;
            }
            if rect.top < self.fullscreen.top {
                shift_y = self.fullscreen.top - rect.top;
            } else if rect.bottom > self.fullscreen.bottom {
                shift_y = self.fullscreen.bottom - rect.bottom;
            }

            if shift_x != 0.0 || shift_y != 0.0 {
                start.x += shift_x;
                start.y += shift_y;
                end.x += shift_x;
                end.y += shift_y;
            }
        }
    }

    /// 根据手柄方向调整选区大小
    fn resize_by(&mut self, handle: Handle, dx: f32, dy: f32) {
        let (Some(ref mut start), Some(ref mut end)) =
            (self.start_pos.as_mut(), self.end_pos.as_mut())
        else {
            return;
        };

        match handle {
            Handle::NW => {
                start.x += dx;
                start.y += dy;
            }
            Handle::N => {
                start.y += dy;
            }
            Handle::NE => {
                start.y += dy;
                end.x += dx;
            }
            Handle::E => {
                end.x += dx;
            }
            Handle::SE => {
                end.x += dx;
                end.y += dy;
            }
            Handle::S => {
                end.y += dy;
            }
            Handle::SW => {
                start.x += dx;
                end.y += dy;
            }
            Handle::W => {
                start.x += dx;
            }
        }
    }

    /// 检测并修正选区翻转：若某轴发生翻转则交换 start/end 对应坐标，并返回重映射后的手柄
    fn fix_flip(&mut self, handle: Handle) -> Option<Handle> {
        let (Some(s), Some(e)) = (self.start_pos.as_mut(), self.end_pos.as_mut()) else {
            return None;
        };
        let mut new_handle = handle;
        let mut flipped = false;

        if s.x > e.x {
            std::mem::swap(&mut s.x, &mut e.x);
            new_handle = new_handle.flip_x();
            flipped = true;
        }
        if s.y > e.y {
            std::mem::swap(&mut s.y, &mut e.y);
            new_handle = new_handle.flip_y();
            flipped = true;
        }

        if flipped {
            Some(new_handle)
        } else {
            None
        }
    }
}

/// 绘制"挖空"效果：在 fullscreen 上画 4 个矩形，留出选区区域
fn draw_cutout(session: &DrawingSession, brush: &Brush, rect: &Rect, fullscreen: &Rect) {
    let Rect { left, top, right, bottom} = *rect;
    let Rect { left: fl, top: ft, right: fr, bottom: fb} = *fullscreen;

    // 左边：x=0 ~ left, 高度 = 全屏高
    let r = Rect::new(fl, ft, left, fb);
    session.fill_rect(&r, brush);
    // 上边：x=left ~ right, 高度 = top
    let r = Rect::new(left, ft, right, top);
    session.fill_rect(&r, brush);
    // 右边：x=right ~ 全屏右, 高度 = 全屏高
    let r = Rect::new(right, ft, fr, fb);
    session.fill_rect(&r, brush);
    // 下边：x=left ~ right, y=bottom ~ 全屏底
    let r = Rect::new(left, bottom, right, fb);
    session.fill_rect(&r, brush);
}

/// 状态机定义
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    /// 初始什么都没，只有全屏遮罩层
    #[default]
    None,
    /// 区域选定好的状态
    Idle,
    /// 区域框选中
    Selecting,
    /// 改变区域大小
    Resize,
    /// 移动区域位置
    Move,
}
