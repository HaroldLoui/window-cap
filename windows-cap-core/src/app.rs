use crate::event::{Action, Event, KeyState, MouseState, SharedEvents, SharedKeys, SharedMouse};
use windows::Win32::*;
use windows_canvas::*;
use windows_window::{Window, WindowBuilder, run_with};

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
    /// - `session`: 绘图会话，框架已 `begin_draw`，用户只需绘制，无需 `present`
    /// - 返回 `Ok(true)` 立即继续渲染（动画模式）
    /// - 返回 `Ok(false)` 进入 idle，等待下一条窗口消息再渲染（节省 CPU）
    /// - 返回 `Err(e)` 由 `run_with` 透传，Rust `Termination` 打印并退出
    fn update(&mut self, ctx: &Ctx, session: &DrawingSession) -> Result<bool>;
}

// ── run_app ──────────────────────────────────────────────────────────

/// 框架入口 — 用户配置窗口、构造业务数据，框架管 device/chain/dcomp/present/事件路由
pub fn run_app<A: App>(
    title: &str,
    configure: impl FnOnce(WindowBuilder) -> WindowBuilder,
    init: impl FnOnce(&Window) -> Result<A>,
) -> Result<()> {
    // 共享状态
    let mouse = SharedMouse::new();
    let keys = SharedKeys::new();
    let events = SharedEvents::new();

    // 用户配置 builder，框架注入 on_message 后 create
    let builder = configure(Window::new(title));
    let window = builder
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
        .create()?;

    // 框架管 device / chain / dcomp
    let device = GpuDevice::new()?;
    let (w, h) = window.client_size();
    let mut chain = device.create_swap_chain_for_window(&window, w as u32, h as u32)?;

    let (_dcomp, _target, _visual) = unsafe {
        let dcomp: IDCompositionDesktopDevice = DCompositionCreateDevice2(device.d2d_device())?;
        let target: IDCompositionTarget = dcomp.CreateTargetForHwnd(HWND(window.hwnd()), true)?;
        let visual: IDCompositionVisual2 = dcomp.CreateVisual()?;
        
        visual.SetContent(chain.raw_swap_chain()).ok()?;
        target.SetRoot(&visual).ok()?;
        dcomp.Commit().ok()?;

        (dcomp, target, visual)
    };

    // 用户构造业务数据
    let mut app = init(&window)?;

    // 帧上下文 + 帧循环（自动 begin_draw / present）
    let mut ctx = Ctx {
        mouse: mouse.clone(),
        keys: keys.clone(),
        events: Vec::new(),
    };

    run_with(move || {
        let frame_events = events.take();
        // begin_draw 之前先按本帧 resize 事件调整 chain
        for e in &frame_events {
            if let Action::Resize { w, h } = e {
                chain.resize(*w as u32, *h as u32)?;
            }
        }
        ctx.events = frame_events;
        let session = chain.begin_draw()?;
        let cont = app.update(&ctx, &session)?;
        drop(session);
        chain.present()?;
        Ok(cont)
    })
}
