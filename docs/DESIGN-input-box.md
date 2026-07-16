# 透明输入框设计方案

> 基于当前 `windows-canvas-demo` 项目，在全屏 overlay 上实现鼠标框选 + IME 中文输入 + 文本绘制。

## 1. 当前状态

```
src/main.rs:
- 全屏 WS_POPUP 窗口（WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP）
- DirectComposition 合成（逐像素 alpha 透明）
- 半透明黑色遮罩 (0,0,0,0.3)
- ESC 退出
- run_with 渲染循环（每帧重绘）
- TODO: 获取鼠标坐标
```

## 2. 依赖变更

```toml
[dependencies]
windows-canvas = { git = "https://github.com/microsoft/windows-rs" }
windows-window = { git = "https://github.com/microsoft/windows-rs" }
windows = { git = "https://github.com/microsoft/windows-rs", features = [
    "Win32_UI_WindowsAndMessaging",       # 已有 — WM_*/WS_* 常量
    "Win32_Graphics_DirectComposition",    # 已有
    "Win32_UI_Input_Ime",                  # 新增 — ImmGetContext/ImmSetCompositionWindow/ImmGetCompositionStringW
] }
```

无需新增 crate，只加一个 feature。

## 3. 状态机

```
┌──────────┐   LBUTTONDOWN    ┌───────────┐   LBUTTONUP    ┌──────────┐
│  Idle    │ ───────────────→ │ Selecting │ ─────────────→ │ Selected │
│ (遮罩)   │                  │ (拖拽框选) │                │ (已框选)  │
└──────────┘                  └───────────┘                └────┬─────┘
                                                                │
                                                     点击框选区域内
                                                                │
                                                                ↓
                              ┌───────────┐   Enter/Escape  ┌──────────┐
                              │  Editing  │ ──────────────→ │ Selected │
                              │ (输入中)   │                 │          │
                              └───────────┘                 └──────────┘
```

### 3.1 Idle 状态

- 全屏半透明遮罩
- 监听 `WM_LBUTTONDOWN` → 记录起点，切换到 Selecting

### 3.2 Selecting 状态

- 监听 `WM_MOUSEMOVE` → 实时更新选区矩形
- 监听 `WM_LBUTTONUP` → 确定选区，切换到 Selected
- 每帧渲染：遮罩 + 实时选区边框

### 3.3 Selected 状态

- 选区"挖空"：在 `session.clear(TRANSPARENT)` 后，只在选区**外部**画半透明遮罩
- 选区边框：白色/蓝色描边
- 监听 `WM_LBUTTONDOWN`：
  - 点在选区内 → 记录输入位置，切换到 Editing
  - 点在选区外 → 忽略（或取消选区回到 Idle）

### 3.4 Editing 状态

- 输入框：在点击位置画边框矩形（无填充，背景透明）
- 光标：闪烁竖线
- 监听 `WM_IME_COMPOSITION`：
  - `GCS_COMPSTR` → 读取组合中文字（有下划线），更新 `composing_text`
  - `GCS_RESULTSTR` → 读取确认文字，追加到 `committed_text`，清空 `composing_text`
- 监听 `WM_CHAR`：
  - 回车 (0x0D) → 确认输入，文本"烧"到画面上，回到 Selected
  - 退格 (0x08) → 删除最后一个字符
  - 其他可打印字符 → 追加到 `committed_text`
- 调用 `ImmSetCompositionWindow` 将 IME 候选框定位到输入框下方

## 4. 核心数据结构

```rust
use std::cell::RefCell;
use windows_canvas::*;

/// 应用状态（在 run_with 闭包外用 Rc<RefCell<>> 共享给 on_message）
struct AppState {
    mode: Mode,

    // 框选
    sel_start: Option<(i32, i32)>,   // LBUTTONDOWN 坐标
    sel_end: Option<(i32, i32)>,     // MOUSEMOVE/LBUTTONUP 坐标
    selection: Option<Rect>,          // 最终选区（DIP）

    // 文本输入
    cursor_pos: Option<(f32, f32)>,  // 输入框位置（DIP）
    committed_text: String,           // 已确认文字
    composing_text: String,           // IME 组合中文字
    cursor_visible: bool,             // 光标闪烁状态
    cursor_tick: u32,                 // 闪烁计数器

    // 已"烧录"到画面上的文字（确认后不再修改）
    burned_texts: Vec<BurnedText>,
}

#[derive(PartialEq)]
enum Mode {
    Idle,
    Selecting,
    Selected,
    Editing,
}

struct BurnedText {
    text: String,
    rect: Rect,       // 绘制区域
    format: TextFormat,
}
```

## 5. 消息处理（on_message 回调）

```rust
// 在 Window::new("overlay").on_message(|hwnd, msg, wp, lp| { ... }) 中

const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP:   u32 = 0x0202;
const WM_MOUSEMOVE:   u32 = 0x0200;

match msg {
    // ── 鼠标 ──
    WM_LBUTTONDOWN => {
        let x = (lp & 0xFFFF) as i16 as i32;       // 低位，有符号
        let y = ((lp >> 16) & 0xFFFF) as i16 as i32; // 高位，有符号
        let mut state = state.borrow_mut();
        match state.mode {
            Mode::Idle => {
                state.sel_start = Some((x, y));
                state.sel_end = Some((x, y));
                state.mode = Mode::Selecting;
            }
            Mode::Selected => {
                // 判断点击是否在选区内
                if let Some(sel) = &state.selection {
                    let fx = x as f32;
                    let fy = y as f32;
                    if fx >= sel.left && fx <= sel.right
                        && fy >= sel.top && fy <= sel.bottom
                    {
                        state.cursor_pos = Some((fx, fy));
                        state.committed_text.clear();
                        state.composing_text.clear();
                        state.mode = Mode::Editing;
                        // 定位 IME 候选框
                        set_ime_position(hwnd, x, y);
                    }
                }
            }
            Mode::Editing => {
                // 点击输入框外部 → 确认当前输入
                // 点击输入框内部 → 移动光标（高级功能，先不实现）
                state.mode = Mode::Selected;
            }
            _ => {}
        }
        Some(0)
    }
    WM_MOUSEMOVE => {
        let x = (lp & 0xFFFF) as i16 as i32;
        let y = ((lp >> 16) & 0xFFFF) as i16 as i32;
        let mut state = state.borrow_mut();
        if state.mode == Mode::Selecting {
            state.sel_end = Some((x, y));
        }
        Some(0)
    }
    WM_LBUTTONUP => {
        let x = (lp & 0xFFFF) as i16 as i32;
        let y = ((lp >> 16) & 0xFFFF) as i16 as i32;
        let mut state = state.borrow_mut();
        if state.mode == Mode::Selecting {
            state.sel_end = Some((x, y));
            // 计算最终选区
            if let (Some((sx, sy)), Some((ex, ey))) = (state.sel_start, state.sel_end) {
                let left = sx.min(ex) as f32;
                let top = sy.min(ey) as f32;
                let right = sx.max(ex) as f32;
                let bottom = sy.max(ey) as f32;
                if (right - left) > 5.0 && (bottom - top) > 5.0 {
                    state.selection = Some(Rect::new(left, top, right, bottom));
                    state.mode = Mode::Selected;
                } else {
                    state.mode = Mode::Idle; // 太小，忽略
                }
            }
        }
        Some(0)
    }

    // ── IME ──
    WM_IME_COMPOSITION => {
        let mut state = state.borrow_mut();
        if state.mode != Mode::Editing {
            return None; // 不在编辑模式，交给默认处理
        }
        let himc = ImmGetContext(hwnd);
        if himc.0 == 0 { return Some(0); }

        // 组合中文字（带下划线的候选文字）
        if (lp as u32) & GCS_COMPSTR.0 != 0 {
            let len = ImmGetCompositionStringW(himc, GCS_COMPSTR, None, 0);
            if len > 0 {
                let mut buf = vec![0u16; (len / 2) as usize];
                ImmGetCompositionStringW(
                    himc, GCS_COMPSTR,
                    Some(buf.as_mut_ptr() as *mut _),
                    len as u32,
                );
                state.composing_text = String::from_utf16_lossy(&buf);
            }
        }

        // 确认文字（用户按空格/回车确认后的最终文字）
        if (lp as u32) & GCS_RESULTSTR.0 != 0 {
            let len = ImmGetCompositionStringW(himc, GCS_RESULTSTR, None, 0);
            if len > 0 {
                let mut buf = vec![0u16; (len / 2) as usize];
                ImmGetCompositionStringW(
                    himc, GCS_RESULTSTR,
                    Some(buf.as_mut_ptr() as *mut _),
                    len as u32,
                );
                let result = String::from_utf16_lossy(&buf);
                state.committed_text.push_str(&result);
                state.composing_text.clear();
            }
        }

        ImmReleaseContext(hwnd, himc);
        Some(0)
    }

    // ── 普通字符（英文、数字、标点） ──
    WM_CHAR => {
        let mut state = state.borrow_mut();
        if state.mode != Mode::Editing {
            return None;
        }
        match wp as u32 {
            0x0D => { // Enter → 确认输入
                burn_text(&mut state);
                state.mode = Mode::Selected;
            }
            0x08 => { // Backspace
                state.committed_text.pop();
            }
            0x1B => { // Escape → 取消输入
                state.committed_text.clear();
                state.composing_text.clear();
                state.mode = Mode::Selected;
            }
            ch if ch >= 0x20 => { // 可打印字符
                if let Some(c) = char::from_u32(ch) {
                    state.committed_text.push(c);
                }
            }
            _ => {}
        }
        Some(0)
    }

    // ESC 退出
    WM_KEYDOWN if wp == 0x1B => {
        let mut state = state.borrow_mut();
        match state.mode {
            Mode::Idle => { quit(); }
            Mode::Editing => { state.mode = Mode::Selected; }
            Mode::Selected => { state.mode = Mode::Idle; state.selection = None; }
            _ => {}
        }
        Some(0)
    }

    _ => None,
}
```

## 6. IME 定位

```rust
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::Ime::{
    ImmGetContext, ImmReleaseContext, ImmSetCompositionWindow,
    COMPOSITIONFORM, CFS_POINT,
};

fn set_ime_position(hwnd: *mut c_void, x: i32, y: i32) {
    unsafe {
        let himc = ImmGetContext(HWND(hwnd));
        if himc.0 == 0 { return; }
        let form = COMPOSITIONFORM {
            dwStyle: CFS_POINT,
            ptCurrentPos: POINT { x, y },
            rcArea: RECT::default(),
        };
        ImmSetCompositionWindow(himc, &form);
        ImmReleaseContext(HWND(hwnd), himc);
    }
}
```

## 7. 渲染逻辑（run_with 闭包）

每帧执行，根据状态绘制不同内容：

```rust
run_with(|| {
    let state = state_ref.borrow();
    let width = chain.width() as f32;
    let height = chain.height() as f32;
    let session = chain.begin_draw()?;
    session.clear(ColorF::TRANSPARENT);

    match state.mode {
        Mode::Idle => {
            // 全屏半透明遮罩
            let brush = session.create_solid_brush(ColorF::new(0.0, 0.0, 0.0, 0.3))?;
            session.fill_rect(&Rect::from_xywh(0.0, 0.0, width, height), &brush);
        }

        Mode::Selecting => {
            // 全屏遮罩
            let brush = session.create_solid_brush(ColorF::new(0.0, 0.0, 0.0, 0.3))?;
            session.fill_rect(&Rect::from_xywh(0.0, 0.0, width, height), &brush);
            // 实时选区边框（选区内部不画遮罩 → 透明 → 露出原画面）
            if let (Some((sx, sy)), Some((ex, ey))) = (state.sel_start, state.sel_end) {
                let sel = Rect::new(
                    sx.min(ex) as f32, sy.min(ey) as f32,
                    sx.max(ex) as f32, sy.max(ey) as f32,
                );
                // 用 TRANSPARENT 清除选区（直接在 DirectComposition 层操作）
                // 实际做法：先画遮罩，再用 D2D layer + geometric clip "挖空"
                draw_overlay_with_hole(&session, width, height, &sel)?;
                // 选区边框
                let border = session.create_solid_brush(ColorF::new(0.2, 0.6, 1.0, 1.0))?;
                session.draw_rect(&sel, &border, 2.0);
            }
        }

        Mode::Selected => {
            if let Some(sel) = &state.selection {
                draw_overlay_with_hole(&session, width, height, sel)?;
                // 选区边框
                let border = session.create_solid_brush(ColorF::new(0.2, 0.6, 1.0, 1.0))?;
                session.draw_rect(sel, &border, 2.0);
                // 已烧录的文字
                for bt in &state.burned_texts {
                    session.draw_text(&bt.text, &bt.format, &bt.rect, &brush);
                }
            }
        }

        Mode::Editing => {
            if let (Some(sel), Some((cx, cy))) = (&state.selection, &state.cursor_pos) {
                draw_overlay_with_hole(&session, width, height, sel)?;
                // 选区边框
                let border = session.create_solid_brush(ColorF::new(0.2, 0.6, 1.0, 1.0))?;
                session.draw_rect(sel, &border, 2.0);

                // 输入框边框
                let input_rect = Rect::from_xywh(*cx, *cy, 300.0, 36.0);
                let input_border = session.create_solid_brush(ColorF::WHITE)?;
                session.draw_rect(&input_rect, &input_border, 1.0);

                // 文字：committed + composing
                let format = TextFormat::new("Microsoft YaHei UI", 20.0)?;
                let text_brush = session.create_solid_brush(ColorF::WHITE)?;
                let display_text = format!("{}{}", state.committed_text, state.composing_text);
                let text_rect = Rect::from_xywh(cx + 4.0, cy + 4.0, 292.0, 28.0);
                session.draw_text(&display_text, &format, &text_rect, &text_brush);

                // 光标
                if state.cursor_visible {
                    // 用 TextLayout 测量文字宽度来定位光标（简化版：固定偏移）
                    let cursor_x = cx + 4.0 + measure_text_width(&display_text, &format);
                    session.draw_line(
                        Vector2::new(cursor_x, cy + 4.0),
                        Vector2::new(cursor_x, cy + 28.0),
                        &text_brush, 1.0,
                    );
                }
            }
        }
    }

    drop(session);
    chain.present()?;
    Ok(true)
})
```

## 8. "挖空"选区的实现

DirectComposition + D2D 的方案：使用 **AxisAlignedClip** 将遮罩裁剪到选区外部。

```rust
/// 在全屏画半透明遮罩，但在 hole 区域"挖空"（不画遮罩 → 透明 → 露出底层画面）
fn draw_overlay_with_hole(
    session: &DrawingSession,
    screen_w: f32,
    screen_h: f32,
    hole: &Rect,
) -> Result<()> {
    let brush = session.create_solid_brush(ColorF::new(0.0, 0.0, 0.0, 0.3))?;

    // 画 4 块遮罩（上、下、左、右），绕过选区
    // 上
    if hole.top > 0.0 {
        session.fill_rect(&Rect::from_xywh(0.0, 0.0, screen_w, hole.top), &brush);
    }
    // 下
    if hole.bottom < screen_h {
        session.fill_rect(&Rect::from_xywh(0.0, hole.bottom, screen_w, screen_h - hole.bottom), &brush);
    }
    // 左
    if hole.left > 0.0 {
        session.fill_rect(&Rect::from_xywh(0.0, hole.top, hole.left, hole.bottom - hole.top), &brush);
    }
    // 右
    if hole.right < screen_w {
        session.fill_rect(&Rect::from_xywh(hole.right, hole.top, screen_w - hole.right, hole.bottom - hole.top), &brush);
    }

    Ok(())
}
```

这个方案最简单，不需要 D2D Layer，直接画 4 块矩形绕过选区。

## 9. 文字宽度测量

`windows-canvas` 没有暴露 `TextLayout` 类型（文档标注为 future work），所以无法精确测量文字宽度来定位光标。有两个方案：

### 方案 A（推荐）：直接调 DirectWrite COM 接口

```rust
fn measure_text_width(text: &str, format: &TextFormat) -> f32 {
    unsafe {
        let factory = dwrite_factory().unwrap();
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let layout = factory.CreateTextLayout(
            &wide,
            text.len() as u32,
            format.raw(),
            10000.0, // 足够大的宽度
            1000.0,
        ).unwrap();
        let mut metrics = DWRITE_TEXT_METRICS::default();
        layout.GetMetrics(&mut metrics).unwrap();
        metrics.widthIncludingTrailingWhitespace
    }
}
```

需要 `dwrite_factory()` —— `windows-canvas` 的 `text.rs` 里有 `pub(crate) fn dwrite_factory()`，
但不是 pub 的。可以自己创建一个 `IDWriteFactory`，或者直接用 `DWriteCreateFactory`。

### 方案 B：字符数 × 估算宽度

简单但不精确（中英文字符宽度不同）。

## 10. 完整文件结构

```
src/
  main.rs        — 窗口创建、消息循环、渲染循环、状态管理
                 （单文件，~250 行，demo 足够）
```

不需要拆模块，保持 demo 简洁。

## 11. 实现步骤

| 步骤 | 内容 | 验证 |
|------|------|------|
| 1 | 加 `Win32_UI_Input_Ime` feature，cargo check | 编译通过 |
| 2 | 实现 `AppState` + 状态机 + 鼠标框选 | 拖拽出选区，松手后选区保留 |
| 3 | 实现 4 块遮罩"挖空" | 选区内透明，可以看到桌面 |
| 4 | 实现 WM_CHAR 输入（先不做 IME） | 英文可以输入并显示 |
| 5 | 实现 WM_IME_COMPOSITION | 中文输入法可用 |
| 6 | 实现光标闪烁 | 光标每 500ms 闪烁 |
| 7 | Enter 确认 + 文字烧录 | 文字固定在画面上 |
| 8 | 调整选区可重新开始 | 完整闭环 |

## 12. 已知限制

| 限制 | 原因 | 影响 |
|------|------|------|
| 文字宽度测量不精确 | `TextLayout` 未 pub | 光标位置可能偏移 |
| 没有文字选中/拖选 | 需要 TextHitTest | 只能逐字符删除 |
| 输入框固定宽度 300px | 简化实现 | 长文本会溢出 |
| 没有滚动 | 单行输入 | 不影响 demo |
| IME 候选框定位依赖 ImmSetCompositionWindow | 系统级 API | 大部分输入法兼容 |
