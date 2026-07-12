use windows_canvas::{Brush, ColorF, DrawingSession, Result};

/// 工具的画笔状态 — 管理 brush 惰性创建、换色、描边宽度
pub struct BrushState {
    pub color: ColorF,
    pub stroke_width: f32,
    brush: Option<Brush>,
}

impl BrushState {
    pub fn new(color: ColorF, width: f32) -> Self {
        Self {
            color,
            stroke_width: width,
            brush: None,
        }
    }

    /// 获取或创建 brush（惰性初始化）
    pub fn brush(&mut self, session: &DrawingSession) -> Result<&Brush> {
        if self.brush.is_none() {
            self.brush = Some(session.create_solid_brush(self.color)?);
        }
        Ok(self.brush.as_ref().unwrap())
    }

    #[allow(dead_code)]
    pub fn set_color(&mut self, color: ColorF) {
        self.color = color;
        if let Some(ref brush) = self.brush {
            brush.set_color(color);
        }
    }

    #[allow(dead_code)]
    pub fn set_stroke_width(&mut self, width: f32) {
        self.stroke_width = width;
    }
}
