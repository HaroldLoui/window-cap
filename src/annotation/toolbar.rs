use windows_canvas::{ColorF, DrawingSession, Rect, Result};

use crate::{annotation::ui, brush::BrushState};

const TOOL_LABELS: &[&str; 10] = &[
    "S","R","C","A","F","T","B","U","S","X"
];

pub struct Toolbar {
    screen_width: f32,
    screen_height: f32,
    brush: BrushState,
}

impl Toolbar {
    pub fn new(width: i32, height: i32) -> Self {
        let brush = BrushState::new(ColorF::RED, 5.0);
        Toolbar {
            screen_width: width as f32,
            screen_height: height as f32,
            brush,
        }
    }

    pub fn draw(&mut self, session: &DrawingSession, selection_bounds: Rect) -> Result<()> {
        let brush = self.brush.brush(session)?;
        let rect = calc_toolbar_rect(selection_bounds, (self.screen_width, self.screen_height));
        ui::draw_toolbar(session, brush, rect);
        for (index, label) in TOOL_LABELS.iter().enumerate() {
            let rect = calc_tool_rect(rect, index);
            ui::draw_tool_icon(session, brush, rect, *label)?;
        }
        Ok(())
    }
}

/// 工具栏宽度（暂时写死）
const TOOLBAR_WIDTH: f32 = 350f32;
/// 工具栏高度
const TOOLBAR_HEIGHT: f32 = 38f32;
/// 工具栏于选区之间的间距
const TOOLBAR_SELECTIION_GAP: f32 = 6f32;

/// 工具栏内工具图标的大小
const TOOL_ICON_SIZE: f32 = 30f32;
/// 工具栏内各工具的间距
const TOOL_GAP: f32 = 3f32;

/// 计算工具栏ui 外边框
fn calc_toolbar_rect(selection_bounds: Rect, screen_size: (f32, f32)) -> Rect {
    // 计算中心
    let cx = selection_bounds.left + selection_bounds.width() / 2.0;
    let mut x = cx - TOOLBAR_WIDTH / 2.0;
    let mut y = selection_bounds.top - TOOLBAR_HEIGHT - TOOLBAR_SELECTIION_GAP;

    // 如果超出屏幕顶部，放到选区内部上沿
    if y < 0.0 {
        y = selection_bounds.top + TOOLBAR_SELECTIION_GAP;
    }

    // 左右不能超出屏幕
    if x < 2.0 {
        x = 2.0;
    }
    if x + TOOLBAR_WIDTH > screen_size.0 - 2.0 {
        x = screen_size.0 - TOOLBAR_WIDTH - 2.0;
    }

    Rect::from_xywh(x, y, TOOLBAR_WIDTH, TOOLBAR_HEIGHT)
}

fn calc_tool_rect(outer_rect: Rect, index: usize) -> Rect {
    let x = outer_rect.left + TOOL_GAP * (index + 1) as f32 + TOOL_ICON_SIZE * index as f32;
    let y = outer_rect.top + TOOL_GAP;

    Rect::from_xywh(x, y, TOOL_ICON_SIZE, TOOL_ICON_SIZE)
} 