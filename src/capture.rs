use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
    GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS,
    BI_RGB, CAPTUREBLT, SRCCOPY,
};
use windows_canvas::{Rect, Result};

/// 截取全屏画面，返回 BGRA 像素数据
///
/// 返回 `(pixels, width, height)`：
/// - `pixels`: 每行 `width * 4` 字节，从上到下排列（top-down）
/// - `width` / `height`: 物理像素尺寸
pub fn capture_screen(width: i32, height: i32) -> Result<(Vec<u8>, i32, i32)> {
    unsafe {
        let screen_dc = GetDC(None);
        if screen_dc.is_invalid() {
            return Err(windows::core::Error::from_hresult(
                windows::core::HRESULT(-1),
            ));
        }

        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
        let old_obj = SelectObject(mem_dc, bitmap.into());

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
                biCompression: BI_RGB.0,
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
        let _ = DeleteObject(bitmap.into());
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
    // clamp 选区到屏幕范围内
    let left = rect.left.max(0.0) as i32;
    let top = rect.top.max(0.0) as i32;
    let right = (rect.right as i32).min(screen_w);
    let bottom = (rect.bottom as i32).min(screen_h);

    let region_w = right - left;
    let region_h = bottom - top;

    if region_w <= 0 || region_h <= 0 {
        return Err(windows::core::Error::from_hresult(
            windows::core::HRESULT(-1),
        ));
    }

    let pitch = screen_w as usize * 4;

    // 裁剪：逐行复制 BGRA → RGBA
    let mut rgba = vec![0u8; (region_w * region_h) as usize * 4];
    for y in 0..region_h {
        let src_offset = (top + y) as usize * pitch + left as usize * 4;
        let dst_offset = y as usize * region_w as usize * 4;
        for x in 0..region_w as usize {
            let si = src_offset + x * 4;
            let di = dst_offset + x * 4;
            rgba[di] = pixels[si + 2];     // R
            rgba[di + 1] = pixels[si + 1]; // G
            rgba[di + 2] = pixels[si];     // B
            rgba[di + 3] = pixels[si + 3]; // A
        }
    }

    if let Err(_) = image::save_buffer(
        path,
        &rgba,
        region_w as u32,
        region_h as u32,
        image::ExtendedColorType::Rgba8,
    ) {
        return Err(windows::core::Error::from_hresult(
            windows::core::HRESULT(-1),
        ));
    }

    Ok(())
}
