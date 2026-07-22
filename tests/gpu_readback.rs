//! 验证 D2D GPU 回读方案：
//! - GetTarget() → ID2D1Image → cast 到 ID2D1Bitmap1
//! - CreateBitmap(CPU_READ | CANNOT_DRAW) 创建 staging bitmap
//! - CopyFromBitmap 从 target 拷贝到 staging
//! - Map(READ) 读回像素
//! - 像素内容与预期一致

use windows::core::Interface;
use windows::Win32::*;
use windows_canvas::{ColorF, GpuDevice, Rect, Result};

/// 最小化测试：创建一个 D2D device + swap chain（离屏，不关联窗口），
/// 画一个已知颜色，然后通过 staging bitmap 读回像素，验证颜色一致。
#[test]
fn gpu_readback_staging_bitmap() -> Result<()> {
    let w = 4u32;
    let h = 4u32;

    // 1. 创建 GPU device + swap chain（离屏模式，不需要 HWND）
    let device = GpuDevice::new()?;
    let mut chain = device.create_swap_chain(w, h)?;

    // 2. 在同一帧内：画红色 + 回读
    let session = chain.begin_draw()?;

    // 画一个红色矩形
    session.clear(ColorF::TRANSPARENT);
    let brush = session.create_solid_brush(ColorF::new(1.0, 0.0, 0.0, 1.0))?;
    session.fill_rect(&Rect::from_xywh(0.0, 0.0, w as f32, h as f32), &brush);

    // cast 到 windows crate 的 ID2D1DeviceContext
    let ctx: ID2D1DeviceContext = session.raw().cast()?;

    // 关键：Flush 确保之前的 clear + fill_rect 命令真正执行到 target
    unsafe { let _ = ctx.Flush(None, None); }

    // 4. GetTarget → cast 到 ID2D1Bitmap1
    let target_image = unsafe { ctx.GetTarget()? };
    let target_bitmap: ID2D1Bitmap1 = target_image.cast()?;
    println!("✅ GetTarget → cast ID2D1Bitmap1 成功");

    // 5. 创建 staging bitmap (CPU_READ | CANNOT_DRAW)
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
    let staging: ID2D1Bitmap1 = unsafe {
        ctx.CreateBitmap(D2D_SIZE_U { width: w, height: h }, None, 0, &props)?
    };
    println!("✅ CreateBitmap(CPU_READ | CANNOT_DRAW) 成功");

    // 6. CopyFromBitmap（在 BeginDraw/EndDraw 之间）
    let hr = unsafe { staging.CopyFromBitmap(None, &target_bitmap, None) };
    println!("✅ CopyFromBitmap 返回: hr = 0x{:08X}", hr.0);
    assert!(hr.is_ok(), "CopyFromBitmap 失败: {:?}", hr);

    // 7. Map(READ) → 读回像素
    let mapped = unsafe { staging.Map(D2D1_MAP_OPTIONS_READ)? };
    println!("✅ Map(READ) 成功, pitch = {}", mapped.pitch);

    // 8. 验证像素：BGRA，红色 = B:0 G:0 R:255 A:255
    let pitch = mapped.pitch as usize;
    let row_bytes = w as usize * 4;
    let mut all_red = true;
    for y in 0..h as usize {
        let row = unsafe {
            std::slice::from_raw_parts(mapped.bits.add(y * pitch), row_bytes)
        };
        for x in 0..w as usize {
            let b = row[x * 4];
            let g = row[x * 4 + 1];
            let r = row[x * 4 + 2];
            let a = row[x * 4 + 3];
            if !(b == 0 && g == 0 && r == 255 && a == 255) {
                all_red = false;
                println!(
                    "  像素 ({},{}) = B:{} G:{} R:{} A:{} — 不是红色！",
                    x, y, b, g, r, a
                );
            }
        }
    }
    assert!(all_red, "❌ 像素不是全红，GPU 回读数据不正确");
    println!("✅ 所有像素验证为红色 (B:0 G:0 R:255 A:255)");

    // 9. Unmap
    let hr = unsafe { staging.Unmap() };
    println!("✅ Unmap 返回: hr = 0x{:08X}", hr.0);

    println!("\n🎉 GPU 回读方案验证通过！");

    Ok(())
}
