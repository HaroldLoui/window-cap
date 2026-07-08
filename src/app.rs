use crate::event::{Action, Event, KeyState, MouseState, SharedEvents, SharedKeys, SharedMouse};
use windows::Win32::UI::{
    HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext},
    WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOPMOST, WS_POPUP},
};
use windows_canvas::*;
use windows_window::{Window, run_with};

// ── Ctx ──────────────────────────────────────────────────────────────

/// 帧上下文 — 每帧传递给 App::update()
pub struct Ctx {
    mouse: SharedMouse,
    keys: SharedKeys,
    events: Vec<Action>,
}

impl Ctx {
    /// 当前鼠标状态快照
    pub fn mouse(&self) -> MouseState {
        self.mouse.get()
    }

    /// 当前键盘状态快照（`keys().is_down(Event::KEY_ESC)` 等）
    pub fn keys(&self) -> KeyState {
        self.keys.get()
    }

    /// 上一帧到本帧之间的事件列表
    pub fn events(&self) -> &[Action] {
        &self.events
    }
}

// ── App trait ────────────────────────────────────────────────────────

/// 应用 trait — 用户实现此 trait 定义业务逻辑
pub trait App {
    /// 每帧调用
    /// - `ctx`: 输入上下文（鼠标状态、键盘状态、事件列表）
    /// - 返回 `Ok(true)` 立即继续渲染（动画模式）
    /// - 返回 `Ok(false)` 进入 idle，等待下一条窗口消息再渲染（节省 CPU）
    /// - 返回 `Err(e)` 由 `run_with` 透传，Rust `Termination` 打印并退出
    fn update(&mut self, ctx: &Ctx) -> Result<bool>;
}

// ── run_app ──────────────────────────────────────────────────────────

/// 框架入口 — 创建窗口并运行事件循环
pub fn run_app<A: App>(
    title: &str,
    init: impl FnOnce(&Window) -> Result<A>,
) -> Result<()> {
    // DPI 感知
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)?; };

    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };

    // 共享状态
    let mouse = SharedMouse::new();
    let keys = SharedKeys::new();
    let events = SharedEvents::new();

    // 创建窗口
    let window = Window::new(title)
        .style(WS_POPUP.0)
        .ex_style((WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP).0)
        .on_message({
            let mouse = mouse.clone();
            let keys = keys.clone();
            let events = events.clone();
            move |hwnd, msg, wparam, lparam| {
                let event = Event::from_raw(hwnd, msg, wparam, lparam);
                mouse.update(&event);
                keys.update(&event);
                events.push(event.action);
                None
            }
        })
        .size(screen_w, screen_h)
        .create()?;

    // 用户初始化
    let mut app = init(&window)?;

    // 帧上下文
    let mut ctx = Ctx {
        mouse: mouse.clone(),
        keys: keys.clone(),
        events: Vec::new(),
    };

    // 帧循环
    run_with(move || {
        ctx.events = events.take();
        app.update(&ctx)
    })
}
