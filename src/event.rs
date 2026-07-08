#![allow(dead_code)]


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
    KeyDown { key: u32 },
    KeyUp { key: u32 },
    MouseMove { x: i32, y: i32 },
    MouseDown { button: MouseButton, x: i32, y: i32 },
    MouseUp { button: MouseButton, x: i32, y: i32 },
    DoubleClick { button: MouseButton, x: i32, y: i32 },
    MouseWheel { delta: i32, x: i32, y: i32 },
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
    // ── Common key codes ────────────────────────────────────────────

    pub const KEY_BACK: u32 = 0x08;
    pub const KEY_TAB: u32 = 0x09;
    pub const KEY_ENTER: u32 = 0x0D;
    pub const KEY_SHIFT: u32 = 0x10;
    pub const KEY_CTRL: u32 = 0x11;
    pub const KEY_ALT: u32 = 0x12;
    pub const KEY_ESC: u32 = 0x1B;
    pub const KEY_SPACE: u32 = 0x20;
    pub const KEY_LEFT: u32 = 0x25;
    pub const KEY_UP: u32 = 0x26;
    pub const KEY_RIGHT: u32 = 0x27;
    pub const KEY_DOWN: u32 = 0x28;
    pub const KEY_DELETE: u32 = 0x2E;

    pub const KEY_A: u32 = 0x41;
    pub const KEY_C: u32 = 0x43;
    pub const KEY_V: u32 = 0x56;
    pub const KEY_X: u32 = 0x58;
    pub const KEY_Z: u32 = 0x5A;

    // ── Constructor ─────────────────────────────────────────────────

    pub fn from_raw(
        hwnd: *mut core::ffi::c_void,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> Self {
        let action = match msg {
            WM_KEYDOWN => Action::KeyDown { key: wparam as u32 },
            WM_KEYUP => Action::KeyUp { key: wparam as u32 },

            WM_MOUSEMOVE => Action::MouseMove {
                x: lo(lparam),
                y: hi(lparam),
            },

            WM_LBUTTONDOWN => Action::MouseDown { button: MouseButton::Left, x: lo(lparam), y: hi(lparam) },
            WM_RBUTTONDOWN => Action::MouseDown { button: MouseButton::Right, x: lo(lparam), y: hi(lparam) },
            WM_MBUTTONDOWN => Action::MouseDown { button: MouseButton::Middle, x: lo(lparam), y: hi(lparam) },

            WM_LBUTTONUP => Action::MouseUp { button: MouseButton::Left, x: lo(lparam), y: hi(lparam) },
            WM_RBUTTONUP => Action::MouseUp { button: MouseButton::Right, x: lo(lparam), y: hi(lparam) },
            WM_MBUTTONUP => Action::MouseUp { button: MouseButton::Middle, x: lo(lparam), y: hi(lparam) },

            WM_LBUTTONDBLCLK => Action::DoubleClick { button: MouseButton::Left, x: lo(lparam), y: hi(lparam) },
            WM_RBUTTONDBLCLK => Action::DoubleClick { button: MouseButton::Right, x: lo(lparam), y: hi(lparam) },
            WM_MBUTTONDBLCLK => Action::DoubleClick { button: MouseButton::Middle, x: lo(lparam), y: hi(lparam) },

            WM_MOUSEWHEEL => {
                let x = lo(lparam);
                let y = hi(lparam);
                let delta = hi_word(wparam as isize) as i32;
                Action::MouseWheel { delta, x, y }
            }

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
    pub fn key(&self) -> Option<u32> {
        match self.action {
            Action::KeyDown { key } | Action::KeyUp { key } => Some(key),
            _ => None,
        }
    }

    /// Cursor position for any mouse-related action.
    pub fn mouse_pos(&self) -> Option<(i32, i32)> {
        match self.action {
            Action::MouseMove { x, y }
            | Action::MouseDown { x, y, .. }
            | Action::MouseUp { x, y, .. }
            | Action::DoubleClick { x, y, .. }
            | Action::MouseWheel { x, y, .. } => Some((x, y)),
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
    pub x: i32,
    pub y: i32,
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
        if let Some((x, y)) = event.mouse_pos() {
            s.x = x;
            s.y = y;
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
    pub fn is_down(&self, key: u32) -> bool {
        if key >= 256 {
            return false;
        }
        let idx = (key / 64) as usize;
        let bit = key % 64;
        self.bits[idx] & (1 << bit) != 0
    }

    fn set(&mut self, key: u32) {
        if key >= 256 {
            return;
        }
        let idx = (key / 64) as usize;
        let bit = key % 64;
        self.bits[idx] |= 1 << bit;
    }

    fn clear(&mut self, key: u32) {
        if key >= 256 {
            return;
        }
        let idx = (key / 64) as usize;
        let bit = key % 64;
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
