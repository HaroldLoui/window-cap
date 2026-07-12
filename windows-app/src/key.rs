/// Windows 虚拟键码（Virtual-Key Codes）
///
/// <https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes>
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    // ── Common ──────────────────────────────────────────────────────
    Back = 0x08,
    Tab = 0x09,
    Enter = 0x0D,
    Pause = 0x13,
    CapsLock = 0x14,
    Escape = 0x1B,
    Space = 0x20,
    PageUp = 0x21,
    PageDown = 0x22,
    End = 0x23,
    Home = 0x24,
    Left = 0x25,
    Up = 0x26,
    Right = 0x27,
    Down = 0x28,
    PrintScreen = 0x2C,
    Insert = 0x2D,
    Delete = 0x2E,

    // ── 0-9 ─────────────────────────────────────────────────────────
    Key0 = 0x30,
    Key1 = 0x31,
    Key2 = 0x32,
    Key3 = 0x33,
    Key4 = 0x34,
    Key5 = 0x35,
    Key6 = 0x36,
    Key7 = 0x37,
    Key8 = 0x38,
    Key9 = 0x39,

    // ── A-Z ─────────────────────────────────────────────────────────
    A = 0x41,
    B = 0x42,
    C = 0x43,
    D = 0x44,
    E = 0x45,
    F = 0x46,
    G = 0x47,
    H = 0x48,
    I = 0x49,
    J = 0x4A,
    K = 0x4B,
    L = 0x4C,
    M = 0x4D,
    N = 0x4E,
    O = 0x4F,
    P = 0x50,
    Q = 0x51,
    R = 0x52,
    S = 0x53,
    T = 0x54,
    U = 0x55,
    V = 0x56,
    W = 0x57,
    X = 0x58,
    Y = 0x59,
    Z = 0x5A,

    // ── Windows ─────────────────────────────────────────────────────
    LeftWin = 0x5B,
    RightWin = 0x5C,
    Apps = 0x5D,

    // ── Numpad ──────────────────────────────────────────────────────
    Numpad0 = 0x60,
    Numpad1 = 0x61,
    Numpad2 = 0x62,
    Numpad3 = 0x63,
    Numpad4 = 0x64,
    Numpad5 = 0x65,
    Numpad6 = 0x66,
    Numpad7 = 0x67,
    Numpad8 = 0x68,
    Numpad9 = 0x69,
    Multiply = 0x6A,
    Add = 0x6B,
    Separator = 0x6C,
    Subtract = 0x6D,
    Decimal = 0x6E,
    Divide = 0x6F,

    // ── Function keys ───────────────────────────────────────────────
    F1 = 0x70,
    F2 = 0x71,
    F3 = 0x72,
    F4 = 0x73,
    F5 = 0x74,
    F6 = 0x75,
    F7 = 0x76,
    F8 = 0x77,
    F9 = 0x78,
    F10 = 0x79,
    F11 = 0x7A,
    F12 = 0x7B,
    F13 = 0x7C,
    F14 = 0x7D,
    F15 = 0x7E,
    F16 = 0x7F,
    F17 = 0x80,
    F18 = 0x81,
    F19 = 0x82,
    F20 = 0x83,
    F21 = 0x84,
    F22 = 0x85,
    F23 = 0x86,
    F24 = 0x87,

    // ── Lock / toggle ───────────────────────────────────────────────
    NumLock = 0x90,
    ScrollLock = 0x91,

    // ── Shift / Ctrl / Alt（左右区分）─────────────────────────────
    LeftShift = 0xA0,
    RightShift = 0xA1,
    LeftControl = 0xA2,
    RightControl = 0xA3,
    LeftAlt = 0xA4,
    RightAlt = 0xA5,

    // ── Browser / media / app ───────────────────────────────────────
    BrowserBack = 0xA6,
    BrowserForward = 0xA7,
    BrowserRefresh = 0xA8,
    BrowserStop = 0xA9,
    BrowserSearch = 0xAA,
    BrowserFavorites = 0xAB,
    BrowserHome = 0xAC,
    VolumeMute = 0xAD,
    VolumeDown = 0xAE,
    VolumeUp = 0xAF,
    MediaNext = 0xB0,
    MediaPrev = 0xB1,
    MediaStop = 0xB2,
    MediaPlayPause = 0xB3,
    LaunchMail = 0xB4,
    LaunchMedia = 0xB5,
    LaunchApp1 = 0xB6,
    LaunchApp2 = 0xB7,

    // ── Punctuation (OEM) ───────────────────────────────────────────
    /// `;:` on US standard keyboard
    Oem1 = 0xBA,
    /// `+` on any keyboard
    OemPlus = 0xBB,
    /// `,` on any keyboard
    OemComma = 0xBC,
    /// `-` on any keyboard
    OemMinus = 0xBD,
    /// `.` on any keyboard
    OemPeriod = 0xBE,
    /// `/?` on US standard keyboard
    Oem2 = 0xBF,
    /// `` `~ `` on US standard keyboard
    Oem3 = 0xC0,
    /// `[{` on US standard keyboard
    Oem4 = 0xDB,
    /// `\|` on US standard keyboard
    Oem5 = 0xDC,
    /// `]}` on US standard keyboard
    Oem6 = 0xDD,
    /// `'"` on US standard keyboard
    Oem7 = 0xDE,
    /// Misc (angle bracket or backslash on RT 102)
    Oem8 = 0xDF,
    /// `<>` on RT 102 or `\` on US
    Oem102 = 0xE2,

    // ── IME ─────────────────────────────────────────────────────────
    ImeProcess = 0xE5,
    Packet = 0xE7,

    // ── Attn / CrSel / ExSel / Erase / Play / Zoom ──────────────────
    Attn = 0xF6,
    CrSel = 0xF7,
    ExSel = 0xF8,
    EraseEof = 0xF9,
    Play = 0xFA,
    Zoom = 0xFB,
    NoName = 0xFC,
    Pa1 = 0xFD,
    OemClear = 0xFE,

    /// 未映射的键码（保留原始值）
    Unknown(u8),
}

impl Key {
    /// 从原始 `u32` 键码构造（通常来自 `wparam`）
    pub fn from_u32(v: u32) -> Self {
        let b = (v & 0xFF) as u8;
        Self::from_u8(b)
    }

    /// 从 `u8` 构造
    pub fn from_u8(v: u8) -> Self {
        match v {
            0x08 => Self::Back,
            0x09 => Self::Tab,
            0x0D => Self::Enter,
            0x13 => Self::Pause,
            0x14 => Self::CapsLock,
            0x1B => Self::Escape,
            0x20 => Self::Space,
            0x21 => Self::PageUp,
            0x22 => Self::PageDown,
            0x23 => Self::End,
            0x24 => Self::Home,
            0x25 => Self::Left,
            0x26 => Self::Up,
            0x27 => Self::Right,
            0x28 => Self::Down,
            0x2C => Self::PrintScreen,
            0x2D => Self::Insert,
            0x2E => Self::Delete,

            0x30 => Self::Key0,
            0x31 => Self::Key1,
            0x32 => Self::Key2,
            0x33 => Self::Key3,
            0x34 => Self::Key4,
            0x35 => Self::Key5,
            0x36 => Self::Key6,
            0x37 => Self::Key7,
            0x38 => Self::Key8,
            0x39 => Self::Key9,

            0x41 => Self::A,
            0x42 => Self::B,
            0x43 => Self::C,
            0x44 => Self::D,
            0x45 => Self::E,
            0x46 => Self::F,
            0x47 => Self::G,
            0x48 => Self::H,
            0x49 => Self::I,
            0x4A => Self::J,
            0x4B => Self::K,
            0x4C => Self::L,
            0x4D => Self::M,
            0x4E => Self::N,
            0x4F => Self::O,
            0x50 => Self::P,
            0x51 => Self::Q,
            0x52 => Self::R,
            0x53 => Self::S,
            0x54 => Self::T,
            0x55 => Self::U,
            0x56 => Self::V,
            0x57 => Self::W,
            0x58 => Self::X,
            0x59 => Self::Y,
            0x5A => Self::Z,

            0x5B => Self::LeftWin,
            0x5C => Self::RightWin,
            0x5D => Self::Apps,

            0x60 => Self::Numpad0,
            0x61 => Self::Numpad1,
            0x62 => Self::Numpad2,
            0x63 => Self::Numpad3,
            0x64 => Self::Numpad4,
            0x65 => Self::Numpad5,
            0x66 => Self::Numpad6,
            0x67 => Self::Numpad7,
            0x68 => Self::Numpad8,
            0x69 => Self::Numpad9,
            0x6A => Self::Multiply,
            0x6B => Self::Add,
            0x6C => Self::Separator,
            0x6D => Self::Subtract,
            0x6E => Self::Decimal,
            0x6F => Self::Divide,

            0x70 => Self::F1,
            0x71 => Self::F2,
            0x72 => Self::F3,
            0x73 => Self::F4,
            0x74 => Self::F5,
            0x75 => Self::F6,
            0x76 => Self::F7,
            0x77 => Self::F8,
            0x78 => Self::F9,
            0x79 => Self::F10,
            0x7A => Self::F11,
            0x7B => Self::F12,
            0x7C => Self::F13,
            0x7D => Self::F14,
            0x7E => Self::F15,
            0x7F => Self::F16,
            0x80 => Self::F17,
            0x81 => Self::F18,
            0x82 => Self::F19,
            0x83 => Self::F20,
            0x84 => Self::F21,
            0x85 => Self::F22,
            0x86 => Self::F23,
            0x87 => Self::F24,

            0x90 => Self::NumLock,
            0x91 => Self::ScrollLock,

            0xA0 => Self::LeftShift,
            0xA1 => Self::RightShift,
            0xA2 => Self::LeftControl,
            0xA3 => Self::RightControl,
            0xA4 => Self::LeftAlt,
            0xA5 => Self::RightAlt,

            0xA6 => Self::BrowserBack,
            0xA7 => Self::BrowserForward,
            0xA8 => Self::BrowserRefresh,
            0xA9 => Self::BrowserStop,
            0xAA => Self::BrowserSearch,
            0xAB => Self::BrowserFavorites,
            0xAC => Self::BrowserHome,
            0xAD => Self::VolumeMute,
            0xAE => Self::VolumeDown,
            0xAF => Self::VolumeUp,
            0xB0 => Self::MediaNext,
            0xB1 => Self::MediaPrev,
            0xB2 => Self::MediaStop,
            0xB3 => Self::MediaPlayPause,
            0xB4 => Self::LaunchMail,
            0xB5 => Self::LaunchMedia,
            0xB6 => Self::LaunchApp1,
            0xB7 => Self::LaunchApp2,

            0xBA => Self::Oem1,
            0xBB => Self::OemPlus,
            0xBC => Self::OemComma,
            0xBD => Self::OemMinus,
            0xBE => Self::OemPeriod,
            0xBF => Self::Oem2,
            0xC0 => Self::Oem3,
            0xDB => Self::Oem4,
            0xDC => Self::Oem5,
            0xDD => Self::Oem6,
            0xDE => Self::Oem7,
            0xDF => Self::Oem8,
            0xE2 => Self::Oem102,

            0xE5 => Self::ImeProcess,
            0xE7 => Self::Packet,

            0xF6 => Self::Attn,
            0xF7 => Self::CrSel,
            0xF8 => Self::ExSel,
            0xF9 => Self::EraseEof,
            0xFA => Self::Play,
            0xFB => Self::Zoom,
            0xFC => Self::NoName,
            0xFD => Self::Pa1,
            0xFE => Self::OemClear,

            other => Self::Unknown(other),
        }
    }

    /// 转回原始 `u32` 键码
    pub fn to_u32(self) -> u32 {
        match self {
            Self::Unknown(v) => v as u32,
            // 所有命名变体的 discriminant 值就是键码本身
            // 无法用 `as u8`，因为 enum 包含带字段的变体 Unknown(u8)，
            // 所以通过 repr(u8) 的内存布局安全地取第一个字节作为 discriminant。
            _ => {
                // SAFETY: repr(u8) 保证 discriminant 存储在第一个字节
                // 且所有命名变体的 discriminant 值就是键码本身
                unsafe { *<*const Self>::from(&self).cast::<u8>() as u32 }
            }
        }
    }
}
