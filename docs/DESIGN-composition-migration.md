# Composition 迁移方案

将渲染架构从 SwapChain + DirectComposition 迁移到 Windows.UI.Composition + CompositionDrawingSurface。

## 背景

当前项目使用 SwapChain 作为帧缓冲，配合 DirectComposition（`IDCompositionDesktopDevice`）将内容嵌入窗口。这套方案适合游戏等连续渲染场景，但对截图工具这类按需更新的应用来说偏重。

`windows-rs` 生态现在提供了 `windows-composition` crate，封装了 `Windows.UI.Composition` API，可以更简洁地实现相同效果。

---

## 架构对比

### 当前方案：SwapChain + DirectComposition

```
┌─────────────────────────────────────────────────────────────┐
│  run_with 帧循环（每帧执行，即使内容未变）                      │
│                                                             │
│  begin_draw()  →  绘制命令  →  drop(session)  →  present()  │
└─────────────────────────────────────────────────────────────┘
         ↓
    SwapChain (back buffer ↔ front buffer，双缓冲翻转)
         ↓
    DComposition Visual (手动桥接，unsafe)
         ↓
    窗口显示
```

**关键代码** (`windows-cap-core/src/app.rs`):

```rust
// 创建（unsafe，手动桥接）
let device = GpuDevice::new()?;
let mut chain = device.create_swap_chain_for_window(&window, w, h)?;

let dcomp: IDCompositionDesktopDevice =
    unsafe { DCompositionCreateDevice2(device.d2d_device())? };
let target = unsafe { dcomp.CreateTargetForHwnd(HWND(window.hwnd()), true)? };
let visual = unsafe { dcomp.CreateVisual()? };
unsafe {
    visual.SetContent(chain.raw_swap_chain())?;
    target.SetRoot(&visual)?;
    dcomp.Commit()?;
}

// 帧循环（每帧执行）
run_with(move || {
    for e in &frame_events {
        if let Action::Resize { w, h } = e {
            chain.resize(*w, *h)?;
        }
    }
    let session = chain.begin_draw()?;
    app.update(&ctx, &session)?;  // 即使无变化也要执行
    drop(session);
    chain.present()?;
    Ok(cont)
});
```

### 新方案：Composition + CompositionDrawingSurface

```
┌─────────────────────────────────────────────────────────────┐
│  run 消息循环（仅分发事件，不主动渲染）                         │
│                                                             │
│  事件到达 → 回调触发 → surface.draw(|session| { ... })       │
└─────────────────────────────────────────────────────────────┘
         ↓
    CompositionDrawingSurface (合成器管理，无显式 present)
         ↓
    CompositionSurfaceBrush → SpriteVisual
         ↓
    系统合成器 (DWM) 统一合成 → 窗口显示
```

**关键代码**（新）:

```rust
// 创建（全 safe API）
let _queue = DispatcherQueueController::create_on_current_thread()?;
let compositor = Compositor::new()?;
let target = compositor.create_desktop_window_target(&window, false)?;
let root = compositor.create_container_visual();
target.set_root(&root);

let device = GpuDevice::new()?;
let graphics = device.create_graphics_device(&compositor)?;
let surface = graphics.create_drawing_surface(w as f32, h as f32)?;

let sprite = compositor.create_sprite_visual();
sprite.set_brush(&compositor.create_surface_brush(&surface));
root.children().insert_at_top(&sprite);

// 按需绘制（仅在内容变化时调用）
surface.draw(|session| {
    session.clear(ColorF::TRANSPARENT);
    app.draw_background(session);
    app.draw_overlay(session);
})?;

// 消息循环（不主动渲染）
run();
```

---

## 逐项差异

| 维度 | SwapChain 方案 | Composition 方案 |
|------|---------------|-----------------|
| **帧控制** | 应用通过 `present()` 控制翻转 | 绘制即提交，合成器控制显示 |
| **空闲行为** | 每帧 `begin_draw/present`，持续 GPU 活动 | 无绘制调用时零 GPU 开销 |
| **重绘触发** | 每帧自动（`run_with`） | 事件驱动，手动调用 `surface.draw()` |
| **API 安全性** | DComposition 部分 unsafe | 全 safe |
| **代码量** | DComposition 桥接 ~15 行 | ~6 行 |
| **依赖** | `windows` crate (dcomp feature) | `windows-composition` crate |
| **窗口样式** | 需要 `WS_EX_NOREDIRECTIONBITMAP` | 可能不需要（合成器直接管理），需测试验证 |
| **像素回读** | 从 SwapChain back buffer | 在 `draw()` 闭包内，同样路径 |

---

## 迁移范围

### 需要修改的文件

| 文件 | 改动内容 |
|------|---------|
| `Cargo.toml` | 添加 `windows-composition` 依赖，`windows-canvas` 启用 `composition` feature |
| `windows-cap-core/Cargo.toml` | 添加 `windows-composition` 依赖 |
| `windows-cap-core/src/app.rs` | 重写 `run_app`：替换 DComposition + SwapChain + 帧循环 |
| `src/main.rs` | 窗口样式调整（去掉 `WS_EX_NOREDIRECTIONBITMAP`） |

### 不需要修改的文件

| 文件 | 原因 |
|------|------|
| `src/app.rs` (Screenshot) | `ensure_bitmap` / `draw_background` / `draw_toolbar` 保持不变 |
| `src/selection/` | 选区逻辑不变，只是重绘触发方式改变 |
| `src/capture.rs` | 像素回读路径不变（在 `draw()` 闭包内调用） |
| `src/brush.rs` | Brush 逻辑不变 |

---

## 新方案可能遇到的问题

### 1. 重绘时机：事件驱动 vs 帧循环

**问题**：当前 `run_with` 每帧自动调用 `app.update()`，选区鼠标拖拽时每帧都会重绘。新方案需要在鼠标事件回调中手动调用 `surface.draw()`。

**风险**：
- 鼠标事件回调是 `on_message` 闭包，需要访问 `surface` 和 `app`
- 闭包间共享可变状态需要 `Rc<RefCell<...>>`（参考 composition/canvas 示例）
- 如果事件处理遗漏，选区拖拽可能不跟手

**应对**：
- 在 `on_message` 回调中检测鼠标事件，触发 `surface.draw()`
- 用 `Rc<RefCell<...>>` 共享 `app` 和 `surface` 到事件闭包
- 参考 `composition/canvas` 示例的 `Scene` 模式

### 2. 像素回读时机

**问题**：`capture_gpu_pixels` 需要在 `BeginDraw/EndDraw` 之间调用。当前代码在 `handle_keys` 中调用，此时处于 `begin_draw` 之后。

**风险**：新方案中 `handle_keys` 需要在 `surface.draw()` 闭包内调用，否则无法获取 D2D context。

**应对**：
- 方案 A：在 `surface.draw()` 闭包内调用 `handle_keys`，回读逻辑不变
- 方案 B：先绘制到 `RenderTarget`，再从 `RenderTarget` 回读（多一次 GPU 复制，不推荐）

### 3. 透明窗口行为

**问题**：当前使用 `WS_EX_NOREDIRECTIONBITMAP` 让窗口内容直接显示，不经过 DWM 重定向表面。新方案用 Composition 的视觉树，合成器直接管理图层。

**风险**：
- `WS_EX_NOREDIRECTIONBITMAP` 在 Composition 方案中可能不再需要，也可能仍然需要——需测试验证
- 选区"挖空"效果（背景透出桌面）的实现方式可能不同

**应对**：
- 新方案中，背景用半透明黑色 `CompositionColorBrush` 覆盖全屏
- 选区"挖空"通过不绘制该区域实现（或用多个矩形遮罩）
- 需要测试验证

### 4. DispatcherQueueController 生命周期

**问题**：`DispatcherQueueController` 必须在当前线程创建，且必须在所有 Composition 对象之后 drop。

**风险**：如果 drop 顺序不对，可能 panic 或 undefined behavior。

**应对**：
- 在 `run_app` 开头创建，作为第一个变量
- 保证它在所有 composition 对象之后 drop（Rust 的 drop 顺序是声明的逆序）

### 5. 窗口创建时机

**问题**：`create_desktop_window_target` 需要一个已存在的 `Window`。当前代码先创建窗口再创建渲染设备。

**风险**：`DispatcherQueueController::create_on_current_thread()` 必须在创建任何 Composition 对象之前调用。

**应对**：
- 顺序：`DispatcherQueueController` → `Window` → `Compositor` → `DesktopWindowTarget` → `GpuDevice` → `CompositionGraphicsDevice` → `CompositionDrawingSurface`

### 6. Resize 处理

**问题**：当前在帧循环中检测 `Action::Resize` 并调用 `chain.resize()`。新方案需要在 resize 事件中调用 `surface.resize()`。

**风险**：resize 事件通过 `on_message` 回调分发，需要在回调中触发 surface 重绘。

**应对**：
- 在 `on_resize` 回调中调用 `surface.resize()` + `surface.draw()`
- 参考 `composition/canvas` 示例的 `Scene::resize`

---

## 依赖变更

### 当前

```toml
[workspace.dependencies]
windows-canvas = { git = "https://github.com/microsoft/windows-rs" }
windows-window = { git = "https://github.com/microsoft/windows-rs" }
windows = { git = "https://github.com/microsoft/windows-rs", features = ["winuser", "dcomp", "d2d", "wingdi"] }
```

### 新

```toml
[workspace.dependencies]
windows-canvas = { git = "https://github.com/microsoft/windows-rs", features = ["composition"] }
windows-composition = { git = "https://github.com/microsoft/windows-rs", features = ["system"] }
windows-window = { git = "https://github.com/microsoft/windows-rs" }
windows = { git = "https://github.com/microsoft/windows-rs", features = ["winuser", "d2d", "wingdi"] }
# dcomp feature 不再需要
```

---

## 实施步骤

1. **依赖变更**：Cargo.toml 添加 `windows-composition`，`windows-canvas` 启用 `composition` feature，移除 `windows` 的 `dcomp` feature
2. **重写 `run_app`**：替换 DComposition + SwapChain + 帧循环为 Composition + 按需绘制 + 消息循环
3. **窗口样式**：去掉 `WS_EX_NOREDIRECTIONBITMAP`（如果不需要）
4. **事件驱动重绘**：在 `on_message` / `on_resize` 回调中触发 `surface.draw()`
5. **像素回读**：确认 `capture_gpu_pixels` 在 `surface.draw()` 闭包内可正常工作
6. **测试验证**：选区拖拽跟手、透明 overlay、保存截图

---

## 参考

- [windows-composition 文档](https://github.com/microsoft/windows-rs/blob/master/docs/crates/windows-composition.md)
- [windows-canvas composition bridge](https://github.com/microsoft/windows-rs/blob/master/docs/crates/windows-canvas.md#getting-started--into-a-composition-surface)
- [composition/canvas 示例](https://github.com/microsoft/windows-rs/tree/master/crates/samples/composition/canvas)
