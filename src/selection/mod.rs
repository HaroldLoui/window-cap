use windows_canvas::Rect;
use windows_cap_core::Pos2;

pub mod handles;
pub mod selection;

pub(crate) use selection::Selection;
pub(crate) use handles::*;

/// 将两个点归一化为标准矩形
pub(crate) fn normalize(a: Pos2, b: Pos2) -> Rect {
    let left = a.x.min(b.x);
    let right = a.x.max(b.x);
    let top = a.y.min(b.y);
    let bottom = a.y.max(b.y);
    Rect::new(left, top, right, bottom)
}