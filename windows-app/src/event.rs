#![allow(dead_code)]


use crate::Pos2;
use crate::key::Key;
use windows::Win32::Foundation::HWND;

// ── Message constants ───────────────────────────────────────────────
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_LBUTTONDBLCLK: u32 = 0x0203;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_RBUTTONDBLCLK: u32 = 0x0206;
const WM_MBUTTONDOWN: u32 = 0x0207;
const WM_MBUTTONUP: u32 = 0x0208;
const WM_MBUTTONDBLCLK: u32 = 0x0209;
const WM_MOUSEWHEEL: u32 = 0x020A;
const WM_SIZE: u32 = 0x0005;

// ── Mouse button ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

// ── Typed action ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Action {
    KeyDown { key: Key },
    KeyUp { key: Key },
    MouseMove { pos: Pos2 },
    MouseDown { button: MouseButton, pos: Pos2 },
    MouseUp { button: MouseButton, pos: Pos2 },
    DoubleClick { button: MouseButton, pos: Pos2 },
    MouseWheel { delta: i32, pos: Pos2 },
    Resize { w: i32, h: i32 },
    Other,
}

// ── Event ───────────────────────────────────────────────────────────

pub struct Event {
    pub hwnd: HWND,
    pub msg: u32,
    pub wparam: usize,
    pub lparam: isize,
    pub action: Action,
}

impl Event {
    // ── Constructor ─────────────────────────────────────────────────
    pub fn from_raw(
        hwnd: *mut core::ffi::c_void,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> Self {
        let action = match msg {
            WM_KEYDOWN => Action::KeyDown { key: Key::from_u32(wparam as u32) },
            WM_KEYUP => Action::KeyUp { key: Key::from_u32(wparam as u32) },

            WM_MOUSEMOVE => Action::MouseMove { pos: pos(lparam) },

            WM_LBUTTONDOWN => Action::MouseDown { button: MouseButton::Left, pos: pos(lparam) },
            WM_RBUTTONDOWN => Action::MouseDown { button: MouseButton::Right, pos: pos(lparam) },
            WM_MBUTTONDOWN => Action::MouseDown { button: MouseButton::Middle, pos: pos(lparam) },

            WM_LBUTTONUP => Action::MouseUp { button: MouseButton::Left, pos: pos(lparam) },
            WM_RBUTTONUP => Action::MouseUp { button: MouseButton::Right, pos: pos(lparam) },
            WM_MBUTTONUP => Action::MouseUp { button: MouseButton::Middle, pos: pos(lparam) },

            WM_LBUTTONDBLCLK => Action::DoubleClick { button: MouseButton::Left, pos: pos(lparam) },
            WM_RBUTTONDBLCLK => Action::DoubleClick { button: MouseButton::Right, pos: pos(lparam) },
            WM_MBUTTONDBLCLK => Action::DoubleClick { button: MouseButton::Middle, pos: pos(lparam) },

            WM_MOUSEWHEEL => {
                let delta = hi_word(wparam as isize) as i32;
                Action::MouseWheel { delta, pos: pos(lparam) }
            }

            WM_SIZE => Action::Resize { w: lo(lparam), h: hi(lparam) },

            _ => Action::Other,
        };

        Self {
            hwnd: HWND(hwnd),
            msg,
            wparam,
            lparam,
            action,
        }
    }

    // ── Convenience helpers ─────────────────────────────────────────

    /// Keyboard virtual-key code (valid for KeyDown / KeyUp).
    pub fn key(&self) -> Option<Key> {
        match self.action {
            Action::KeyDown { key } | Action::KeyUp { key } => Some(key),
            _ => None,
        }
    }

    /// Cursor position for any mouse-related action.
    pub fn mouse_pos(&self) -> Option<Pos2> {
        match self.action {
            Action::MouseMove { pos }
            | Action::MouseDown { pos, .. }
            | Action::MouseUp { pos, .. }
            | Action::DoubleClick { pos, .. }
            | Action::MouseWheel { pos, .. } => Some(pos),
            _ => None,
        }
    }

    /// Mouse button for button-related actions.
    pub fn mouse_button(&self) -> Option<MouseButton> {
        match self.action {
            Action::MouseDown { button, .. }
            | Action::MouseUp { button, .. }
            | Action::DoubleClick { button, .. } => Some(button),
            _ => None,
        }
    }

    /// Wheel delta (positive = forward, negative = backward).
    pub fn delta(&self) -> Option<i32> {
        match self.action {
            Action::MouseWheel { delta, .. } => Some(delta),
            _ => None,
        }
    }

    pub fn is_key_down(&self) -> bool {
        matches!(self.action, Action::KeyDown { .. })
    }

    pub fn is_key_up(&self) -> bool {
        matches!(self.action, Action::KeyUp { .. })
    }

    pub fn is_mouse_move(&self) -> bool {
        matches!(self.action, Action::MouseMove { .. })
    }

    pub fn is_mouse_down(&self) -> bool {
        matches!(self.action, Action::MouseDown { .. })
    }

    pub fn is_mouse_up(&self) -> bool {
        matches!(self.action, Action::MouseUp { .. })
    }

    pub fn is_double_click(&self) -> bool {
        matches!(self.action, Action::DoubleClick { .. })
    }

    pub fn is_mouse_wheel(&self) -> bool {
        matches!(self.action, Action::MouseWheel { .. })
    }
}

// ── Private helpers ─────────────────────────────────────────────────

/// Extract the low-order 16 bits of `v` as a signed i32 (x coordinate).
fn lo(v: isize) -> i32 {
    (v as i16) as i32
}

/// Extract the high-order 16 bits of `v` as a signed i32 (y coordinate).
fn hi(v: isize) -> i32 {
    ((v >> 16) as i16) as i32
}

/// Extract mouse position from lparam as Pos2.
fn pos(lparam: isize) -> Pos2 {
    Pos2::new(lo(lparam) as f32, hi(lparam) as f32)
}

/// Extract the high-order 16 bits of `v` as signed i16 (wheel delta).
fn hi_word(v: isize) -> i16 {
    (v >> 16) as i16
}

// ═══════════════════════════════════════════════════════════════════════
// Shared state types
// ═══════════════════════════════════════════════════════════════════════

use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ── MouseState + SharedMouse ─────────────────────────────────────────

/// 鼠标快照状态（Copy，适合 Cell 存储）
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseState {
    pub pos: Pos2,
    pub left: bool,
    pub right: bool,
    pub middle: bool,
}

/// 线程内共享的鼠标状态包装器
pub struct SharedMouse(Rc<Cell<MouseState>>);

impl SharedMouse {
    pub fn new() -> Self {
        Self(Rc::new(Cell::new(MouseState::default())))
    }

    pub fn get(&self) -> MouseState {
        self.0.get()
    }

    pub fn update(&self, event: &Event) {
        let mut s = self.0.get();
        if let Some(pos) = event.mouse_pos() {
            s.pos = pos;
        }
        match &event.action {
            Action::MouseDown { button, .. } => match button {
                MouseButton::Left => s.left = true,
                MouseButton::Right => s.right = true,
                MouseButton::Middle => s.middle = true,
            },
            Action::MouseUp { button, .. } => match button {
                MouseButton::Left => s.left = false,
                MouseButton::Right => s.right = false,
                MouseButton::Middle => s.middle = false,
            },
            Action::DoubleClick { button, .. } => match button {
                MouseButton::Left => s.left = true,
                MouseButton::Right => s.right = true,
                MouseButton::Middle => s.middle = true,
            },
            // MouseWheel: 跳过（屏幕坐标，非客户区坐标）
            // KeyDown/KeyUp/Other: 不更新鼠标状态
            _ => {}
        }
        self.0.set(s);
    }
}

impl Clone for SharedMouse {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

// ── KeyState + SharedKeys ────────────────────────────────────────────

/// 键盘快照状态：256 位 bitset 跟踪虚拟键码 0..=255 的按下状态
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyState {
    bits: [u64; 4],
}

impl KeyState {
    pub fn is_down(&self, key: Key) -> bool {
        let v = key.to_u32();
        if v >= 256 {
            return false;
        }
        let idx = (v / 64) as usize;
        let bit = v % 64;
        self.bits[idx] & (1 << bit) != 0
    }

    fn set(&mut self, key: Key) {
        let v = key.to_u32();
        if v >= 256 {
            return;
        }
        let idx = (v / 64) as usize;
        let bit = v % 64;
        self.bits[idx] |= 1 << bit;
    }

    fn clear(&mut self, key: Key) {
        let v = key.to_u32();
        if v >= 256 {
            return;
        }
        let idx = (v / 64) as usize;
        let bit = v % 64;
        self.bits[idx] &= !(1 << bit);
    }
}

/// 线程内共享的键盘状态包装器
pub struct SharedKeys(Rc<Cell<KeyState>>);

impl SharedKeys {
    pub fn new() -> Self {
        Self(Rc::new(Cell::new(KeyState::default())))
    }

    pub fn get(&self) -> KeyState {
        self.0.get()
    }

    pub fn update(&self, event: &Event) {
        let mut s = self.0.get();
        match &event.action {
            Action::KeyDown { key } => s.set(*key),
            Action::KeyUp { key } => s.clear(*key),
            _ => {}
        }
        self.0.set(s);
    }
}

impl Clone for SharedKeys {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

// ── SharedEvents ─────────────────────────────────────────────────────

/// 事件缓冲区：收集上一帧到本帧之间发生的所有事件
pub struct SharedEvents(Rc<RefCell<Vec<Action>>>);

impl SharedEvents {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Vec::new())))
    }

    pub fn push(&self, action: Action) {
        self.0.borrow_mut().push(action);
    }

    pub fn take(&self) -> Vec<Action> {
        std::mem::take(&mut *self.0.borrow_mut())
    }
}

impl Clone for SharedEvents {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}
