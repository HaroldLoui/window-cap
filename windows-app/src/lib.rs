mod app;
mod event;
mod key;

use std::fmt::Display;

pub use app::{App, Ctx, run_app};
pub use event::{Action, Event, KeyState, MouseButton, MouseState};
pub use key::Key;

use windows_canvas::Vector2;

/// 屏幕坐标点（像素坐标，f32）
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Pos2 {
    pub x: f32,
    pub y: f32,
}

impl Pos2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// 转为绘制 API 所需的 Vector2
    pub fn to_vec2(self) -> Vector2 {
        Vector2::new(self.x, self.y)
    }
}

impl Display for Pos2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

impl From<(i32, i32)> for Pos2 {
    fn from((x, y): (i32, i32)) -> Self {
        Self { x: x as f32, y: y as f32 }
    }
}
