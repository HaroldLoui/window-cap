use windows::Win32::UI::WindowsAndMessaging::*;
use windows_canvas::{Brush, DrawingSession, Rect};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Handle {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

#[derive(Clone, Copy)]
pub struct HandleRect {
    pub handle: Handle,
    pub rect: Rect,
}

impl HandleRect {
    pub fn draw(&self, session: &DrawingSession, brush: &Brush) {
        session.fill_rect(&self.rect, brush);
    }

    pub fn get_cursor_style(&self) -> CursorStyle {
        match self.handle {
            Handle::NW | Handle::SE => CursorStyle::SizeNWSE,
            Handle::NE | Handle::SW => CursorStyle::SizeNESW,
            Handle::N | Handle::S => CursorStyle::SizeNS,
            Handle::E | Handle::W => CursorStyle::SizeWE,
        }
    }
}

const HANDLE_SIZE: f32 = 10.0;
const HANDLE_HALF: f32 = HANDLE_SIZE / 2.0;

pub fn calc_handles(x: f32, y: f32, width: f32, height: f32) -> [HandleRect; 8] {
    let cx = x + width / 2.0;
    let cy = y + height / 2.0;

    [
        HandleRect {
            handle: Handle::NW,
            rect: Rect {
                left: x - HANDLE_HALF,
                top: y - HANDLE_HALF,
                right: x + HANDLE_HALF,
                bottom: y + HANDLE_HALF,
            },
        },
        HandleRect {
            handle: Handle::N,
            rect: Rect {
                left: cx - HANDLE_HALF,
                top: y - HANDLE_HALF,
                right: cx + HANDLE_HALF,
                bottom: y + HANDLE_HALF,
            },
        },
        HandleRect {
            handle: Handle::NE,
            rect: Rect {
                left: x + width - HANDLE_HALF,
                top: y - HANDLE_HALF,
                right: x + width + HANDLE_HALF,
                bottom: y + HANDLE_HALF,
            },
        },
        HandleRect {
            handle: Handle::E,
            rect: Rect {
                left: x + width - HANDLE_HALF,
                top: cy - HANDLE_HALF,
                right: x + width + HANDLE_HALF,
                bottom: cy + HANDLE_HALF,
            },
        },
        HandleRect {
            handle: Handle::SE,
            rect: Rect {
                left: x + width - HANDLE_HALF,
                top: y + height - HANDLE_HALF,
                right: x + width + HANDLE_HALF,
                bottom: y + height + HANDLE_HALF,
            },
        },
        HandleRect {
            handle: Handle::S,
            rect: Rect {
                left: cx - HANDLE_HALF,
                top: y + height - HANDLE_HALF,
                right: cx + HANDLE_HALF,
                bottom: y + height + HANDLE_HALF,
            },
        },
        HandleRect {
            handle: Handle::SW,
            rect: Rect {
                left: x - HANDLE_HALF,
                top: y + height - HANDLE_HALF,
                right: x + HANDLE_HALF,
                bottom: y + height + HANDLE_HALF,
            },
        },
        HandleRect {
            handle: Handle::W,
            rect: Rect {
                left: x - HANDLE_HALF,
                top: cy - HANDLE_HALF,
                right: x + HANDLE_HALF,
                bottom: cy + HANDLE_HALF,
            },
        },
    ]
}

// 获取对应的鼠标样式
pub enum CursorStyle {
    Arrow,
    Cross,
    SizeAll,
    SizeNWSE,
    SizeNESW,
    SizeNS,
    SizeWE,
}

pub fn set_cursor(style: CursorStyle) {
    let id = match style {
        CursorStyle::Arrow => IDC_ARROW,
        CursorStyle::Cross => IDC_CROSS,
        CursorStyle::SizeAll => IDC_SIZEALL,
        CursorStyle::SizeNWSE => IDC_SIZENWSE,
        CursorStyle::SizeNESW => IDC_SIZENESW,
        CursorStyle::SizeNS => IDC_SIZENS,
        CursorStyle::SizeWE => IDC_SIZEWE,
    };
    unsafe {
        let cursor = LoadCursorW(None, id).ok();
        SetCursor(cursor);
    }
}
