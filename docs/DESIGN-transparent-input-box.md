# 透明输入框设计方案

> 在全屏 overlay 上实现：点击定位 → 透明输入框（带边框） → IME 中文输入 → draw_text 绘制

## 1. 当前状态

```
src/main.rs:
├── 全屏 WS_POPUP 窗口（WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP）
├── DirectComposition 合成（逐像素 alpha 透明）
├── 半透明黑色遮罩 (0,0,0,0.3) 覆盖全屏
├── ESC 退出
├── run_with 渲染循环（每帧重绘）
└── TODO: 需要在此处获取鼠标左键按下时的坐标点和松开时的坐标点
```

## 2. 依赖变更

```toml
# 仅需新增一个 feature，无需新 crate
windows = { ..., features = [
    "Win32_UI_WindowsAndMessaging",       # 已有
    "Win32_Graphics_DirectComposition",    # 已有
    "Win32_UI_Input_Ime",                  # ← 新增：IME API
] }
```

**新增 feature 提供的 API：**
- `ImmGetContext(hwnd) -> HIMC` — 获取窗口的 IME 上下文
- `ImmSetCompositionWindow(himc, &COMPOSITIONFORM)` — 定位 IME 候选框
- `ImmGetCompositionStringW(himc, GCS_COMPSTR/GCS_RESULTSTR, ...)` — 读取组合文字
- `ImmReleaseContext(hwnd, himc)` — 释放 IME 上下文

## 3. 状态机设计

```
┌─────────┐  LBUTTONDOWN   ┌───────────┐  LBUTTONUP   ┌──────────┐
│  Idle   │ ─────────────→ │ Selecting │ ───────────→ │ Selected │
│ (遮罩)  │                │ (拖拽框选) │              │ (已框选)  │
└─────────┘                └───────────┘              └────┬─────┘
    ↑                                                       │
    │                                              点击框选区域内
    │ 右键/ESC 取消                                            │
    │                                                       ↓
    │                       ┌───────────┐  Enter/ESC  ┌──────────┐
    └────────────────────── │  Editing  │ ←────────── │ 输入确认  │
                            │ (输入中)   │              │          │
                            └───────────┘              └──────────┘
```

### 各状态行为：

| 状态 | 鼠标点击 | 键盘 | 渲染内容 |
|------|---------|------|---------|
| Idle | 记录起点 → Selecting | ESC 退出 | 全屏半透明遮罩 |
| Selecting | 拖拽更新选区 | ESC → Idle | 遮罩 + 选区边框（选区内透明） |
| Selected | 点在选区内 → Editing | ESC → Idle | 遮罩 + 选区边框 + 已确认文字 |
| Editing | 点在选区外 → Selected | IME/字符输入 / Enter确认 / ESC取消 | 遮罩 + 选区边框 + 输入框 + 文字 + 光标 |

## 4. 核心数据结构

```rust
use std::cell::RefCell;
use std::rc::Rc;
use windows_canvas::*;

/// 应用全局状态（通过 Rc<RefCell<>> 在 on_message 和 run_with 间共享）
struct AppState {
    // ── 模式 ──
    mode: Mode,

    // ── 框选 ──
    selecting: bool,
    sel_start: (i32, i32),       // LBUTTONDOWN 坐标（像素）
    sel_end: (i32, i32),         // MOUSEMOVE/LBUTTONUP 坐标（像素）
    selection: Option<Rect>,      // 最终选区（DIP）

    // ── 文本输入 ──
    input_pos: Option<(f32, f32)>,   // 输入框左上角（DIP）
    buffer: String,                   // 已确认文字
    composing: String,                // IME 组合中文字（带下划线）
    cursor_visible: bool,             // 光标闪烁状态
    cursor_timer: u32,                // 闪烁计数（帧数）

    // ── 已确认文字 ──
    confirmed_texts: Vec<(String, Rect)>,  // (文字, 绘制区域)
}

#[derive(Clone, PartialEq)]
enum Mode {
    Idle,       // 全屏遮罩，等待点击
    Selecting,  // 正在拖拽选区
    Selected,   // 选区确定，等待点击输入
    Editing,    // 正在输入文字
}
```

## 5. 消息处理（on_message 回调）

### 5.1 鼠标消息

```rust
// 在 on_message 闭包中
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP:   u32 = 0x0202;
const WM_MOUSEMOVE:   u32 = 0x0200;

// 提取坐标（lparam 低位= x, 高位= y，有符号 16-bit）
fn get_cursor_pos(lparam: isize) -> (i32, i32) {
    let x = (lparam & 0xFFFF) as i16 as i32;
    let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}
```

**Idle 状态：**
- `WM_LBUTTONDOWN` → 记录 `sel_start`，切换到 Selecting

**Selecting 状态：**
- `WM_MOUSEMOVE` → 更新 `sel_end`
- `WM_LBUTTONUP` → 计算选区矩形，切换到 Selected

**Selected 状态：**
- `WM_LBUTTONDOWN` → 判断点击是否在选区内，是则记录 `input_pos`，切换到 Editing
- 在切换前调用 `set_ime_position()` 将 IME 候选框定位到输入框位置

**Editing 状态：**
- `WM_LBUTTONDOWN` 在选区外 → 确认当前输入，回到 Selected

### 5.2 IME 消息处理

```rust
const WM_IME_COMPOSITION: u32 = 0x010F;
const WM_IME_STARTCOMPOSITION: u32 = 0x010D;
const WM_IME_ENDCOMPOSITION: u32 = 0x010E;

// WM_IME_COMPOSITION 处理逻辑：
if msg == WM_IME_COMPOSITION {
    let hwnd = HWND(window.hwnd() as _);
    let himc = unsafe { ImmGetContext(hwnd) };

    if !himc.is_invalid() {
        // 检查 lparam 是否包含 GCS_COMPSTR（组合中的文字）
        if (lparam as u32) & GCS_COMPSTR.0 != 0 {
            // 读取组合文字
            let len = unsafe { ImmGetCompositionStringW(himc, GCS_COMPSTR, None, 0) };
            if len > 0 {
                let mut buf = vec![0u16; (len / 2) as usize];
                unsafe {
                    ImmGetCompositionStringW(
                        himc, GCS_COMPSTR,
                        Some(buf.as_mut_ptr() as *mut _), len as u32,
                    );
                }
                state.composing = String::from_utf16_lossy(&buf);
            }
        }

        // 检查 lparam 是否包含 GCS_RESULTSTR（已确认的文字）
        if (lparam as u32) & GCS_RESULTSTR.0 != 0 {
            let len = unsafe { ImmGetCompositionStringW(himc, GCS_RESULTSTR, None, 0) };
            if len > 0 {
                let mut buf = vec![0u16; (len / 2) as usize];
                unsafe {
                    ImmGetCompositionStringW(
                        himc, GCS_RESULTSTR,
                        Some(buf.as_mut_ptr() as *mut _), len as u32,
                    );
                }
                let result = String::from_utf16_lossy(&buf);
                state.buffer.push_str(&result);
                state.composing.clear();
            }
        }

        unsafe { ImmReleaseContext(hwnd, himc) };
    }
    return Some(0);
}
```

### 5.3 IME 候选框定位

```rust
/// 将 IME 候选框定位到输入框位置
fn set_ime_position(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let himc = ImmGetContext(hwnd);
        if !himc.is_invalid() {
            let form = COMPOSITIONFORM {
                dwStyle: CFS_POINT,
                ptCurrentPos: POINT { x, y },
                rcArea: RECT::default(),
            };
            ImmSetCompositionWindow(himc, &form);
            ImmReleaseContext(hwnd, himc);
        }
    }
}
```

### 5.4 字符消息（WM_CHAR）— 退格/回车

```rust
const WM_CHAR: u32 = 0x0102;

if msg == WM_CHAR && state.mode == Mode::Editing {
    match wparam as u32 {
        0x08 => { state.buffer.pop(); }           // Backspace
        0x0D => { confirm_input(&mut state); }     // Enter → 确认
        0x1B => { cancel_input(&mut state); }      // Escape → 取消
        code if code >= 0x20 => {                  // 可打印字符
            if let Some(c) = char::from_u32(code) {
                state.buffer.push(c);
            }
        }
        _ => {}
    }
    return Some(0);
}
```

## 6. 渲染逻辑（run_with 闭包）

每帧执行，根据 `state.mode` 绘制不同内容。

### 6.1 Idle / Selecting — 全屏遮罩 + 选区预览

```rust
fn render_overlay(session: &DrawingSession, w: f32, h: f32) {
    // 半透明黑色遮罩覆盖全屏
    let brush = session.create_solid_brush(ColorF::new(0.0, 0.0, 0.0, 0.3)).unwrap();
    session.fill_rect(&Rect::from_xywh(0.0, 0.0, w, h), &brush);
}

fn render_selection_border(session: &DrawingSession, sel: &Rect) {
    // 选区边框（蓝色，2px）
    let border = session.create_solid_brush(ColorF::new(0.3, 0.6, 1.0, 1.0)).unwrap();
    session.draw_rect(sel, &border, 2.0);
}
```

### 6.2 Selected — 挖空选区 + 边框 + 已确认文字

"挖空" = 选区内不画遮罩，露出底层桌面。

```rust
fn render_overlay_with_hole(session: &DrawingSession, w: f32, h: f32, hole: &Rect) {
    let brush = session.create_solid_brush(ColorF::new(0.0, 0.0, 0.0, 0.3)).unwrap();

    // 画 4 块遮罩，绕过选区
    // 上
    session.fill_rect(&Rect::from_xywh(0.0, 0.0, w, hole.top), &brush);
    // 下
    session.fill_rect(&Rect::from_xywh(0.0, hole.bottom, w, h - hole.bottom), &brush);
    // 左
    session.fill_rect(&Rect::from_xywh(0.0, hole.top, hole.left, hole.bottom - hole.top), &brush);
    // 右
    session.fill_rect(&Rect::from_xywh(hole.right, hole.top, w - hole.right, hole.bottom - hole.top), &brush);
}
```

> **为什么用 4 块矩形而不是 D2D Layer？**
> D2D Layer 需要 `PushLayer` + geometric clip，调用链更复杂。
> 4 块矩形方案简单直接，性能完全够用（只有 4 个 fill_rect 调用）。

### 6.3 Editing — 输入框 + 文字 + 光标

```rust
fn render_input_box(session: &DrawingSession, state: &AppState) {
    let (cx, cy) = state.input_pos.unwrap();
    let box_w = 300.0_f32;
    let box_h = 36.0_f32;

    // 1. 输入框边框（白色，1px）
    let border = session.create_solid_brush(ColorF::WHITE).unwrap();
    let input_rect = Rect::from_xywh(cx, cy, box_w, box_h);
    session.draw_rect(&input_rect, &border, 1.0);

    // 2. 文字内容 = 已确认 + 组合中
    let display = format!("{}{}", state.buffer, state.composing);
    if !display.is_empty() {
        let format = TextFormat::new("Microsoft YaHei UI", 20.0).unwrap();
        let text_brush = session.create_solid_brush(ColorF::WHITE).unwrap();
        let text_rect = Rect::from_xywh(cx + 4.0, cy + 4.0, box_w - 8.0, box_h - 8.0);
        session.draw_text(&display, &format, &text_rect, &text_brush);
    }

    // 3. 光标（每 30 帧切换可见性 ≈ 500ms @ 60fps）
    if state.cursor_visible {
        let cursor_brush = session.create_solid_brush(ColorF::WHITE).unwrap();
        // 简化：光标固定在文字末尾（实际应该测量文字宽度）
        let cursor_x = cx + 4.0;
        session.draw_line(
            Vector2::new(cursor_x, cy + 4.0),
            Vector2::new(cursor_x, cy + box_h - 4.0),
            &cursor_brush, 1.0,
        );
    }
}
```

### 6.4 光标闪烁

在 `run_with` 闭包中每帧递增计数器：

```rust
state.cursor_timer += 1;
if state.cursor_timer >= 30 {  // 30帧 ≈ 500ms @ 60fps
    state.cursor_timer = 0;
    state.cursor_visible = !state.cursor_visible;
}
```

## 7. 完整渲染流程

```rust
run_with(|| {
    let mut state = state_ref.borrow_mut();

    // 光标闪烁
    if state.mode == Mode::Editing {
        state.cursor_timer += 1;
        if state.cursor_timer >= 30 {
            state.cursor_timer = 0;
            state.cursor_visible = !state.cursor_visible;
        }
    }

    let w = chain.width() as f32;
    let h = chain.height() as f32;
    let session = chain.begin_draw()?;
    session.clear(ColorF::TRANSPARENT);

    match &state.mode {
        Mode::Idle => {
            render_overlay(&session, w, h);
        }
        Mode::Selecting => {
            render_overlay(&session, w, h);
            let sel = Rect::new(
                state.sel_start.0.min(state.sel_end.0) as f32,
                state.sel_start.1.min(state.sel_end.1) as f32,
                state.sel_start.0.max(state.sel_end.0) as f32,
                state.sel_start.1.max(state.sel_end.1) as f32,
            );
            render_selection_border(&session, &sel);
        }
        Mode::Selected | Mode::Editing => {
            if let Some(sel) = &state.selection {
                render_overlay_with_hole(&session, w, h, sel);
                render_selection_border(&session, sel);
                // 已确认文字
                for (text, rect) in &state.confirmed_texts {
                    let fmt = TextFormat::new("Microsoft YaHei UI", 20.0).unwrap();
                    let brush = session.create_solid_brush(ColorF::WHITE).unwrap();
                    session.draw_text(text, &fmt, rect, &brush);
                }
            }
            if state.mode == Mode::Editing {
                render_input_box(&session, &state);
            }
        }
    }

    drop(session);
    chain.present()?;
    Ok(true)
});
```

## 8. 文字宽度测量（光标定位）

当前 `windows-canvas` 的 `TextFormat` 不暴露 `TextLayout`，无法直接测量文字宽度。

**方案 A（推荐）：直接调 DirectWrite COM**

```rust
use windows::Win32::Graphics::DirectWrite::*;

fn measure_text_width(text: &str, format: &TextFormat) -> f32 {
    unsafe {
        let factory: IDWriteFactory = {
            let mut f = None;
            DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, &IDWriteFactory::IID,
                &mut f as *mut _ as *mut _).ok();
            f.unwrap()
        };
        let wide: Vec<u16> = text.encode_utf16().collect();
        let layout = factory.CreateTextLayout(
            &wide, wide.len() as u32, format.raw(), 1000.0, 100.0,
        ).unwrap();
        let mut metrics = DWRITE_TEXT_METRICS::default();
        layout.GetMetrics(&mut metrics).unwrap();
        metrics.widthIncludingTrailingWhitespace
    }
}
```

需要在 Cargo.toml 中加 `"Win32_Graphics_DirectWrite"` feature。

**方案 B（简化）：字符数 × 估算宽度**

```rust
fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    let char_count: f32 = text.chars().map(|c| {
        if c.is_ascii() { 0.5 } else { 1.0 }  // 中文≈全角，英文≈半角
    }).sum();
    char_count * font_size * 0.6
}
```

不精确但 demo 足够用，且零依赖。

## 9. 完整消息处理流程图

```
on_message(hwnd, msg, wparam, lparam)
│
├─ msg == WM_LBUTTONDOWN (0x0201)
│  ├─ Idle    → sel_start = pos, mode = Selecting
│  ├─ Selected → if pos in selection:
│  │              input_pos = pos, mode = Editing
│  │              set_ime_position(hwnd, x, y)
│  └─ Editing  → 确认当前输入, mode = Selected
│
├─ msg == WM_MOUSEMOVE (0x0200)
│  └─ Selecting → sel_end = pos
│
├─ msg == WM_LBUTTONUP (0x0202)
│  └─ Selecting → selection = normalize(sel_start, sel_end), mode = Selected
│
├─ msg == WM_IME_COMPOSITION (0x010F)
│  └─ Editing →
│     ├─ GCS_COMPSTR  → composing = 读取组合文字
│     └─ GCS_RESULTSTR → buffer += 确认文字, composing.clear()
│
├─ msg == WM_IME_STARTCOMPOSITION (0x010D)
│  └─ Editing → set_ime_position(hwnd, input_pos.x, input_pos.y)
│
├─ msg == WM_CHAR (0x0102)
│  └─ Editing →
│     ├─ 0x08 (Backspace) → buffer.pop()
│     ├─ 0x0D (Enter)     → confirm, mode = Selected
│     ├─ 0x1B (Escape)    → cancel, mode = Selected
│     └─ 其他             → buffer.push(char)
│
├─ msg == WM_KEYDOWN (0x0100)
│  └─ wparam == VK_ESCAPE (0x1B)
│     ├─ Editing  → cancel, mode = Selected
│     ├─ Selected → mode = Idle, selection = None
│     └─ Idle     → quit()
│
└─ 其他 → None（交给默认处理）
```

## 10. 实现步骤

| 步骤 | 内容 | 验证 |
|------|------|------|
| 1 | 加 `Win32_UI_Input_Ime` feature，cargo check | 编译通过 |
| 2 | 实现 `AppState` 结构体 + 状态机骨架 | 编译通过 |
| 3 | 实现鼠标框选（Idle → Selecting → Selected） | 拖拽出选区，松手后选区保留 |
| 4 | 实现 4 块遮罩"挖空" | 选区内透明，可以看到桌面 |
| 5 | 实现选区内点击进入编辑模式 | 点击后出现输入框 |
| 6 | 实现 WM_CHAR 输入（英文） | 英文可以输入并显示 |
| 7 | 实现 WM_IME_COMPOSITION（中文） | 中文输入法可用 |
| 8 | 实现光标闪烁 | 光标每 500ms 闪烁 |
| 9 | Enter 确认 + 文字烧录到画面 | 文字固定在选区内 |

## 11. 已知限制

| 限制 | 原因 | 影响 |
|------|------|------|
| 文字宽度测量需额外 feature 或估算 | `TextLayout` 未 pub | 光标位置可能偏移 |
| 没有文字选中/拖选 | 需要 TextHitTest | 只能逐字符删除 |
| 输入框固定宽度 300px | 简化实现 | 长文本会溢出 |
| 单行输入 | 不支持换行 | 不影响 demo |
| 组合文字没有下划线样式 | draw_text 不支持 per-range 样式 | 用户可能分不清组合 vs 已确认 |
| IME 候选框定位依赖系统 | ImmSetCompositionWindow | 大部分输入法兼容 |

---

## 12. 保存挖空区域为图片（待实现）

> `windows-canvas` 官方文档明确将 **Saving** 和 **Pixel access** 列为 Missing（优先级 medium）。
> 等官方实现后再替换，目前方案为手动从 GPU 读回像素。

### 12.1 原理

保存挖空区域 = 从 swap chain back buffer 读取指定区域的像素 → 写入 PNG 文件。
屏幕上的遮罩、边框等内容不受影响，保存只是"截图"操作。

```
┌─ swap chain（全屏渲染）────────────────────┐
│  遮罩  遮罩  遮罩  遮罩  遮罩  遮罩        │
│  遮罩 ┌──────────────┐ 遮罩  遮罩        │
│  遮罩 │  挖空区域      │ 遮罩  遮罩        │
│  遮罩 │  文字+线段     │ 遮罩  遮罩        │
│  遮罩 └──────────────┘ 遮罩  遮罩        │
│  遮罩  遮罩  遮罩  遮罩  遮罩  遮罩        │
└───────────────────────────────────────────┘

保存时：只读取挖空区域那块像素 → image::save_buffer() → output.png
```

### 12.2 新增依赖

```toml
# Cargo.toml
[dependencies]
image = { version = "0.25", default-features = false, features = ["png"] }

windows = { ..., features = [
    # ... 已有的 feature ...
    "Win32_Graphics_Dxgi",        # ← 新增：IDXGISurface
    "Win32_Graphics_Direct3D11",  # ← 新增：ID3D11Device, ID3D11Texture2D, staging
] }
```

### 12.3 实现：从 swap chain 读取指定区域像素

```rust
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Direct3D11::*;

/// 从 swap chain back buffer 读取指定区域的 BGRA 像素
///
/// # Safety
/// 调用前需确保当前帧已完成 present（或在 present 前、EndDraw 后调用）
fn read_region_pixels(
    chain: &SwapChain,
    device: &GpuDevice,
    x: u32, y: u32, w: u32, h: u32,
) -> Vec<u8> {
    unsafe {
        // 1. 从 swap chain 拿到 back buffer（IDXGISurface）
        //    注意：raw_swap_chain() 返回 canvas crate 的 IDXGISwapChain1，
        //    需要 transmute 成 windows crate 的类型来调用 GetBuffer
        let sc: &IDXGISwapChain1 = std::mem::transmute(chain.raw_swap_chain());
        let back_buffer: ID3D11Texture2D = sc.GetBuffer(0).unwrap();

        // 2. 创建 CPU 可读的 staging texture
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: D3D11_BIND_FLAG(0),
            CPUAccessFlags: D3D11_CPU_ACCESS_READ,
            MiscFlags: D3D11_RESOURCE_MISC_FLAG(0),
        };
        let mut staging: Option<ID3D11Texture2D> = None;
        device.d3d_device().CreateTexture2D(&desc, None, Some(&mut staging)).unwrap();
        let staging = staging.unwrap();

        // 3. 从 back buffer 拷贝指定区域到 staging
        let region = D3D11_BOX {
            left: x, top: y, front: 0,
            right: x + w, bottom: y + h, back: 1,
        };
        let ctx = device.d3d_device().GetImmediateContext().unwrap();
        ctx.CopySubresourceRegion(
            &staging, 0, 0, 0, 0,
            &back_buffer, 0,
            Some(&region),
        );

        // 4. Map 读取像素字节
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        ctx.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).unwrap();

        let stride = mapped.RowPitch as usize;
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h as usize {
            let src = (mapped.pData as *const u8).add(row * stride);
            pixels.extend_from_slice(std::slice::from_raw_parts(src, (w * 4) as usize));
        }

        ctx.Unmap(&staging, 0);
        pixels
    }
}
```

### 12.4 保存为 PNG

```rust
fn save_region_as_png(
    chain: &SwapChain,
    device: &GpuDevice,
    region: &Rect,
    path: &str,
) {
    let x = region.left as u32;
    let y = region.top as u32;
    let w = region.width() as u32;
    let h = region.height() as u32;

    let pixels = read_region_pixels(chain, device, x, y, w, h);

    // swap chain 格式是 BGRA，image crate 需要 Bgra8
    image::save_buffer(path, &pixels, w, h, image::ExtendedColorType::Bgra8).unwrap();
}
```

### 12.5 注意事项

| 事项 | 说明 |
|------|------|
| **transmute 跨 crate** | canvas crate 和 windows crate 各自定义了 `IDXGISwapChain1`，底层 COM 接口相同，transmute 安全但不优雅 |
| **BGRA vs RGBA** | swap chain 格式是 `DXGI_FORMAT_B8G8R8A8_UNORM`，用 `ExtendedColorType::Bgra8` |
| **时机** | 在 `present()` 之后读取（back buffer 内容已确定） |
| **性能** | staging texture 的 Map 是同步操作，会阻塞 GPU pipeline；仅在保存时调用，不在每帧渲染中使用 |
| **官方替换** | 等 `windows-canvas` 实现 `SaveToFile` / pixel access 后，可移除这部分 unsafe 代码 |

### 12.6 与挖空区域的联动

保存时直接传入 `selection` rect：

```rust
// 用户按 Ctrl+S 保存
if let Some(sel) = &state.selection {
    save_region_as_png(&chain, &device, sel, "output.png");
}
```

挖空区域大小/位置变化时，`selection` 自动更新，下次保存用新的 rect 即可。
