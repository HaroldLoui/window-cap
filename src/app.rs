use std::cell::RefCell;
use std::sync::mpsc::{self, Receiver};

use windows::core::Interface;
use windows::Win32::{GetSystemMetrics, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SM_CXSCREEN, SM_CYSCREEN};
use windows_canvas::{Bitmap, DrawingSession, Rect, Result};
use windows_cap_core::{Key, KeyState};
use windows_window::quit;

use crate::annotation::Toolbar;
use crate::capture;
use crate::selection::Selection;
use crate::selection::selection::State;

/// 截图应用 — 冻帧底图 + 选区交互 + 保存输出
pub struct Screenshot {
    /// GDI 截屏的 BGRA 像素数据（CPU 内存，用于保存）
    pixels: Vec<u8>,
    /// 截屏宽度（物理像素）
    width: i32,
    /// 截屏高度（物理像素）
    height: i32,
    /// 底图 bitmap（首帧惰性创建）
    bitmap: Option<Bitmap>,
    /// 挖空选区工具
    pub selection: Selection,
    /// 挖空选区工具
    pub toolbar: Toolbar,
    /// 异步保存完成通知通道
    save_done: RefCell<Option<Receiver<()>>>,
}

impl Screenshot {
    pub fn new(pixels: Vec<u8>, width: i32, height: i32) -> Self {
        Self {
            pixels,
            width,
            height,
            bitmap: None,
            selection: Selection::new(Rect::from_xywh(0.0, 0.0, width as f32, height as f32)),
            toolbar: Toolbar::new(width, height),
            save_done: RefCell::new(None),
        }
    }

    /// 处理键盘事件
    pub fn handle_keys(&self, keys: KeyState, session: &DrawingSession) -> Result<()> {
        // 退出时判断下当前是否有保存任务，没有立即退出，有则让保存任务自己退出
        if keys.is_down(Key::Escape) && self.save_done.borrow().is_none() {
            quit();
            return Ok(());
        }

        // 保存截图到文件系统
        if keys.is_down(Key::Enter) {
            if let Some(rect) = self.selection.bounds() {
                // 主线程：快速回读 GPU 像素（~1-5ms）
                let ctx = session.raw().cast()?;
                let gpu_pixels = capture::capture_gpu_pixels(
                    &ctx,
                    self.width as u32,
                    self.height as u32,
                )?;

                // 异步线程：耗时的裁剪+编码+写文件
                let (tx, rx) = mpsc::channel();
                let w = self.width;
                let h = self.height;
                std::thread::spawn(move || {
                    let _ = capture::save_region(&gpu_pixels, w, h, &rect, "output.png");
                    let _ = tx.send(());
                });

                // 存储通道，后续帧检查完成状态
                *self.save_done.borrow_mut() = Some(rx);
            }
        }

        Ok(())
    }

    /// 检查异步保存是否完成，完成则退出
    pub fn check_save_done(&self) {
        if self.save_done.borrow().as_ref().is_some_and(|rx| rx.try_recv().is_ok()) {
            // 保存完成，退出
            quit();
        }
    }

    /// 惰性创建 bitmap（首帧调用）
    pub fn ensure_bitmap(&mut self, session: &DrawingSession) -> Result<()> {
        if self.bitmap.is_some() {
            return Ok(());
        }

        let bmp = session.create_bitmap(
            &self.pixels,
            self.width as u32,
            self.height as u32,
        )?;
        self.bitmap = Some(bmp);
        Ok(())
    }

    /// 绘制全屏底图
    pub fn draw_background(&self, session: &DrawingSession) {
        let Some(bmp) = &self.bitmap else {
            return;
        };

        let dest_rect = Rect::from_xywh(0.0, 0.0, self.width as f32, self.height as f32);
        session.draw_bitmap(bmp, &dest_rect, 1.0);
    }

    /// 工具栏绘制
    pub fn draw_toolbar(&mut self, session: &DrawingSession) -> Result<()> {
        let se = &self.selection;
        if let Some(bounds) = se.bounds() && se.state() == State::Idle {
            self.toolbar.draw(session, bounds)?;
        }

        Ok(())
    }
}


pub fn get_screen_size() -> (i32, i32) {
    // DPI 感知（必须在 GetSystemMetrics 之前，框架统一处理）
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    };
    let w = unsafe { GetSystemMetrics(SM_CXSCREEN as i32) };
    let h = unsafe { GetSystemMetrics(SM_CYSCREEN as i32) };
    (w, h)
}