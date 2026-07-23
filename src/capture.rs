use windows::core::Interface;
use windows::Win32::*;
use windows_canvas::{Rect, Result};

fn windows_error() -> windows::core::Error {
    windows::core::Error::from_hresult(windows::core::HRESULT(-1))
}

/// 截取全屏画面，返回 BGRA 像素数据
///
/// 返回 `(pixels, width, height)`：
/// - `pixels`: 每行 `width * 4` 字节，从上到下排列（top-down）
/// - `width` / `height`: 物理像素尺寸
pub fn capture_screen(width: i32, height: i32) -> Result<(Vec<u8>, i32, i32)> {
    unsafe {
        let screen_dc = GetDC(None);
        if screen_dc.0.is_null() {
            return Err(windows_error());
        }

        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
        let old_obj = SelectObject(mem_dc, HGDIOBJ(bitmap.0));

        let _ = BitBlt(
            mem_dc, 0, 0, width, height,
            Some(screen_dc), 0, 0,
            SRCCOPY | CAPTUREBLT,
        );

        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // 负值 = top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..Default::default()
            },
            ..Default::default()
        };

        let pixel_count = (width * height) as usize;
        let mut pixels = vec![0u8; pixel_count * 4];

        GetDIBits(
            mem_dc,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &info as *const _ as *mut _,
            DIB_RGB_COLORS,
        );

        SelectObject(mem_dc, old_obj);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(mem_dc);
        ReleaseDC(None, screen_dc);

        Ok((pixels, width, height))
    }
}

/// 从全屏 BGRA 像素中裁剪选区区域，保存为 PNG
///
/// - `pixels`: `capture_screen` 返回的 BGRA 数据
/// - `screen_w` / `screen_h`: 全屏尺寸（物理像素）
/// - `rect`: 选区矩形
/// - `path`: 输出文件路径
pub fn save_region(
    pixels: &[u8],
    screen_w: i32,
    screen_h: i32,
    rect: &Rect,
    path: &str,
) -> Result<()> {
    use std::fs::File;
    use std::io::BufWriter;

    // clamp 选区到屏幕范围内
    let left = rect.left.max(0.0) as i32;
    let top = rect.top.max(0.0) as i32;
    let right = (rect.right as i32).min(screen_w);
    let bottom = (rect.bottom as i32).min(screen_h);

    let region_w = right - left;
    let region_h = bottom - top;

    if region_w <= 0 || region_h <= 0 {
        return Err(windows_error());
    }

    let pitch = screen_w as usize * 4;

    // 裁剪：逐行复制 BGRA → RGBA
    let mut rgba = vec![0u8; (region_w * region_h) as usize * 4];
    for y in 0..region_h {
        let src_offset = (top + y) as usize * pitch + left as usize * 4;
        let dst_offset = (y * region_w) as usize * 4;
        for x in 0..region_w as usize {
            let si = src_offset + x * 4;
            let di = dst_offset + x * 4;
            rgba[di] = pixels[si + 2];     // R
            rgba[di + 1] = pixels[si + 1]; // G
            rgba[di + 2] = pixels[si];     // B
            rgba[di + 3] = pixels[si + 3]; // A
        }
    }

    // PNG 编码：Fast 压缩，优先速度
    let file = File::create(path).map_err(|_| windows_error())?;

    let mut encoder = png::Encoder::new(BufWriter::new(file), region_w as u32, region_h as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    encoder.set_filter(png::FilterType::Up);

    let mut writer = encoder.write_header().map_err(|_| windows_error())?;

    writer.write_image_data(&rgba).map_err(|_| windows_error())?;

    Ok(())
}

/// 从 D2D device context 读回当前渲染结果（BGRA 像素）
///
/// 必须在 BeginDraw/EndDraw 之间调用。内部会 flush 绘制命令。
/// 返回 top-down BGRA 像素，逐行连续，无行对齐 padding。
pub fn capture_gpu_pixels(ctx: &ID2D1DeviceContext, width: u32, height: u32) -> Result<Vec<u8>> {
    unsafe {
        // 0. 关键：Flush 确保之前的 clear + fill_rect 命令真正执行到 target
        let _ = ctx.Flush(None, None);
        
        // 1. 获取当前渲染目标
        let target_image = ctx.GetTarget()?;
        let target_bitmap: ID2D1Bitmap1 = target_image.cast()?;

        // 2. 创建 CPU 可读的 staging bitmap
        //    CPU_READ 要求 CANNOT_DRAW，且不能与其他 flag 组合
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
            ..Default::default()
        };

        let size = D2D_SIZE_U { width, height };
        let staging: ID2D1Bitmap1 = ctx.CreateBitmap(size, None, 0, &props)?;

        // 3. GPU->GPU 复制（隐含 flush，保证之前绘制完成）
        staging.CopyFromBitmap(None, &target_bitmap, None).ok()?;

        // 4. GPU->CPU 映射读取
        let mapped = staging.Map(D2D1_MAP_OPTIONS_READ)?;

        // 5. 逐行拷贝到 Vec<u8>（处理 pitch，去掉行尾对齐 padding）
        let pitch = mapped.pitch as usize;
        let (width, height) = (width as usize, height as usize);
        let src_pixels = std::slice::from_raw_parts(mapped.bits, pitch * height);
        let mut pixels = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            let row_start = y * pitch;
            pixels.extend_from_slice(&src_pixels[row_start..row_start + width * 4]);
        }

        // 6. 解除映射
        staging.Unmap().ok()?;

        Ok(pixels)
    }
}
