use windows::core::Interface;
use windows::d2d::{
    D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_PROPERTIES1, D2D1_INTERPOLATION_MODE_LINEAR,
    ID2D1Bitmap1, ID2D1DeviceContext,
};
use windows::dcommon::{D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U};
use windows::dxgi::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::windef::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2;
use windows::winuser::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN, SetProcessDpiAwarenessContext};
use windows_canvas::{DrawingSession, Rect, Result};
use windows_cap_core::{Key, KeyState};
use windows_window::quit;

use crate::capture;
use crate::selection::Selection;

/// 截图应用 — 冻帧底图 + 选区交互 + 保存输出
pub struct Screenshot {
    /// GDI 截屏的 BGRA 像素数据（CPU 内存，用于保存）
    pixels: Vec<u8>,
    /// 截屏宽度（物理像素）
    width: i32,
    /// 截屏高度（物理像素）
    height: i32,
    /// D2D GPU bitmap（首帧惰性创建）
    bitmap: Option<ID2D1Bitmap1>,
    /// 挖空选区工具
    pub selection: Selection,
}

impl Screenshot {
    pub fn new(pixels: Vec<u8>, width: i32, height: i32) -> Self {
        Self {
            pixels,
            width,
            height,
            bitmap: None,
            selection: Selection::new(Rect::from_xywh(0.0, 0.0, width as f32, height as f32)),
        }
    }

    /// 处理键盘事件，返回是否继续
    pub fn handle_keys(&self, keys: KeyState, session: &DrawingSession) -> Result<bool> {
        if keys.is_down(Key::Escape) {
            quit();
            return Ok(false);
        }

        if keys.is_down(Key::Enter) {
            if let Some(rect) = self.selection.bounds() {
                self.save_region(&rect, "output.png", session)?;
                quit();
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// 惰性创建 D2D bitmap（首帧调用）
    pub fn ensure_bitmap(&mut self, session: &DrawingSession) -> Result<()> {
        if self.bitmap.is_some() {
            return Ok(());
        }

        let ctx: ID2D1DeviceContext = session.raw().cast()?;

        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            ..Default::default()
        };

        let bmp = unsafe {
            ctx.CreateBitmap(
                D2D_SIZE_U {
                    width: self.width as u32,
                    height: self.height as u32,
                },
                Some(self.pixels.as_ptr() as *const _),
                self.width as u32 * 4,
                &props,
            )?
        };
        self.bitmap = Some(bmp);
        Ok(())
    }

    /// 绘制全屏底图
    pub fn draw_background(&self, session: &DrawingSession) {
        let Some(bmp) = &self.bitmap else {
            return;
        };

        let ctx: ID2D1DeviceContext = match session.raw().cast() {
            Ok(c) => c,
            Err(_) => return,
        };

        let dest_rect = D2D_RECT_F {
            left: 0.0,
            top: 0.0,
            right: self.width as f32,
            bottom: self.height as f32,
        };

        unsafe {
            ctx.DrawBitmap(
                bmp,
                Some(&dest_rect),
                1.0,
                D2D1_INTERPOLATION_MODE_LINEAR,
                None,
                None,
            );
        }
    }

    /// 裁剪选区区域保存为 PNG
    pub fn save_region(&self, rect: &Rect, path: &str, session: &DrawingSession) -> Result<()> {
        let ctx: ID2D1DeviceContext = session.raw().cast()?;
        let gpu_pixels = capture::capture_gpu_pixels(
            &ctx, 
            self.width as u32, 
            self.height as u32
        )?;

        capture::save_region(&gpu_pixels, self.width, self.height, rect, path)
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