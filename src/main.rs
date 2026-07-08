mod app;
mod event;

use app::{App, Ctx, run_app};
use event::{Action, Event};
use windows::Win32::{
    Foundation::HWND,
    Graphics::DirectComposition::{DCompositionCreateDevice2, IDCompositionDesktopDevice, IDCompositionTarget, IDCompositionVisual},
};
use windows_canvas::*;
use windows_window::quit;

/// 应用状态 — self 就是 state
struct MyApp {
    chain: SwapChain,
    width: f32,
    height: f32,
    // DirectComposition（必须保持存活）
    _dcomp: IDCompositionDesktopDevice,
    _target: IDCompositionTarget,
    _visual: IDCompositionVisual,
}

impl App for MyApp {
    fn update(&mut self, ctx: &Ctx) -> Result<bool> {
        // ── 按键语义（应用层决策，框架不强制）──
        if ctx.keys().is_down(Event::KEY_ESC) {
            quit();
            return Ok(false);
        }

        // ── 处理瞬时事件 ──
        for event in ctx.events() {
            match event {
                Action::MouseDown { button, x, y } => {
                    println!("pressed {:?} at ({}, {})", button, x, y);
                }
                Action::MouseUp { button, x, y } => {
                    println!("released {:?} at ({}, {})", button, x, y);
                }
                Action::MouseWheel { delta, .. } => {
                    println!("wheel delta: {}", delta);
                }
                _ => {}
            }
        }

        let mouse = ctx.mouse();
        let _ = mouse; // 用 mouse.x, mouse.y, mouse.left 等做绘制

        // ── 绘制 ──
        let session = self.chain.begin_draw()?;
        session.clear(ColorF::TRANSPARENT);

        let brush = session.create_solid_brush(ColorF::new(0.0, 0.0, 0.0, 0.3))?;
        let full_rect = Rect::new(0.0, 0.0, self.width, self.height);
        session.fill_rect(&full_rect, &brush);

        drop(session);
        self.chain.present()?;
        Ok(true)
    }
}

fn main() -> Result<()> {
    run_app("Overlay", |window| {
        let device = GpuDevice::new()?;
        let (w, h) = window.client_size();
        let chain = device.create_swap_chain(w as u32, h as u32)?;

        let dcomp: IDCompositionDesktopDevice = unsafe {
            DCompositionCreateDevice2(device.d2d_device())?
        };
        let target = unsafe { dcomp.CreateTargetForHwnd(HWND(window.hwnd()), true)? };
        let visual = unsafe { dcomp.CreateVisual()? };
        unsafe {
            visual.SetContent(chain.raw_swap_chain())?;
            target.SetRoot(&visual)?;
            dcomp.Commit()?;
        }

        Ok(MyApp {
            chain,
            width: w as f32,
            height: h as f32,
            _dcomp: dcomp,
            _target: target,
            _visual: visual.into(),
        })
    })
}
