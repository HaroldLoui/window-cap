# GPU 回读保存选区（含标注）方案 —— D2D CPU_READ Staging Bitmap

> 替代 `docs/save_region_with_tool.md`，作为最终实施方案

## 1. 问题回顾

```
当前保存路径 (CPU only):
  GDI 截屏 pixels: Vec<u8> → 裁剪选区 → BGRA→RGBA → PNG
  ↑ 没有标注，因为标注是 D2D 画的，不在这条路径上

目标:
  D2D 画完底图 + 遮罩 + 标注 → 读回 GPU 像素 → 裁剪选区 → PNG
```

## 2. 方案选型结论

| 方案 | 结论 | 原因 |
|------|------|------|
| A: CPU 重绘标注 | ❌ 放弃 | 每种标注写两套（D2D + CPU），文字/抗锯齿复杂，长期不可维护 |
| C: 直接 Map back buffer | ❌ 不可行 | `CPU_READ` 不能和 `TARGET` 组合，微软 API 限制 |
| C': 离屏拷贝 + Map | ✅ **采用** | 标注只写一套 D2D，保存 = 所见即所得，性能好 |
| A+Trait 折中 | ❌ 放弃 | 仍然两套实现，不一致风险 |
| D3D11 Staging Texture | ⚠️ 备选 | 和 C' 原理相同，但需要 `d3d11`/`d3dcommon` 特性，更复杂，无额外收益 |
| WIC Bitmap RenderTarget | ❌ 放弃 | 需要独立渲染路径，与 ID2D1DeviceContext 不兼容 |
| CommandList 回放 | ❌ 放弃 | 需要重构 App::update 架构，过度设计 |

## 3. 核心方案：D2D CPU_READ Staging Bitmap

### 3.1 原理

利用 D2D 1.1 的 `ID2D1Bitmap1::Map` API，创建一个不可绘制但可 CPU 读取的 staging bitmap：

```
D2D 绘制完（底图 + 遮罩 + 标注，仍在 BeginDraw/EndDraw 之间）
  ↓
ctx.GetTarget() → 拿到当前 target bitmap（swap chain backbuffer）
  ↓
ctx.CreateBitmap(CPU_READ | CANNOT_DRAW) → 创建 staging bitmap（CPU 可读，GPU 显存）
  ↓
staging.CopyFromBitmap(target)            → GPU→GPU 显存复制（隐含 flush）
  ↓
staging.Map(READ)                         → GPU→CPU 映射（返回 pitch + bits 指针）
  ↓
逐行拷贝 bits → Vec<u8>                  → CPU 拿到 BGRA 像素
  ↓
staging.Unmap()
  ↓
裁剪选区 → BGRA→RGBA → PNG 编码（复用现有逻辑）
```

### 3.2 为什么可以在 EndDraw 之前调用

这是本文档纠正 `save_region_with_tool.md` 的关键点：

- `CopyFromBitmap` **可以**在 `BeginDraw`/`EndDraw` 之间调用。MSDN: "Calling this method may cause the current batch to flush if the bitmap is active in the batch." —— 它内部会刷新 D2D 命令批次，保证之前的所有绘制完成后再复制。
- Staging bitmap 用 `CPU_READ | CANNOT_DRAW` 创建，**不是**渲染目标，所以没有 `TARGET` 和 `CPU_READ` 的 flag 冲突。
- `Map` 也是用在 staging bitmap 上，和渲染目标无关。

所以整个操作可以在 `App::update()` 内部完成，**无需修改帧循环，无需 before_present 回调**。

### 3.3 时机选择

```
当前帧循环:
  update(&mut self, ctx, session) {
    handle_keys()            ← 用户按 Enter，触发保存
    ensure_bitmap()
    selection.draw()
    // ★ 在这里读回 GPU 像素 ★
    // session 仍然活跃（BeginDraw 之后，EndDraw 之前）
    // 所有 D2D 绘制已完成
  }
  drop(session)               ← EndDraw
  chain.present()             ← Present

我们的方案:
  在 update() 末尾，所有 draw 调用之后：
    1. ctx = session.raw().cast()  → ID2D1DeviceContext
    2. 创建 staging bitmap
    3. CopyFromBitmap + Map → Vec<u8>
    4. PNG 编码 + 写文件
    5. 返回 Ok(false) 退出
```

关键：**不需要额外的帧**。按下 Enter 的当前帧就能完成保存。

### 3.4 代码实现

#### `capture.rs` 新增函数

```rust
/// 从 D2D device context 读回当前渲染结果（BGRA 像素）
///
/// 必须在 BeginDraw/EndDraw 之间调用。内部会 flush 绘制命令。
/// 返回 top-down BGRA 像素，逐行连续，无行对齐 padding。
pub fn capture_gpu_pixels(
    ctx: &ID2D1DeviceContext,
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    unsafe {
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

        // 3. GPU→GPU 复制（隐含 flush，保证之前绘制完成）
        staging.CopyFromBitmap(None, &target_bitmap, None)?;

        // 4. GPU→CPU 映射读取
        let mapped = staging.Map(D2D1_MAP_OPTIONS_READ)?;

        // 5. 逐行拷贝到 Vec<u8>（处理 pitch，去掉行尾对齐 padding）
        let pitch = mapped.pitch as usize;
        let src_pixels = std::slice::from_raw_parts(mapped.bits, pitch * height as usize);
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height as usize {
            let row_start = y * pitch;
            pixels.extend_from_slice(&src_pixels[row_start..row_start + width as usize * 4]);
        }

        // 6. 解除映射
        staging.Unmap()?;

        Ok(pixels)
    }
}
```

#### `main.rs` 修改

```rust
// 在 Screenshot 中：
fn save_region(
    &self,
    rect: &Rect,
    path: &str,
    session: &DrawingSession,
) -> Result<()> {
    let ctx: ID2D1DeviceContext = session.raw().cast()?;

    // GPU 回读：包含底图 + 遮罩 + 标注，所见即所得
    let gpu_pixels = capture::capture_gpu_pixels(
        &ctx,
        self.width as u32,
        self.height as u32,
    )?;

    // 复用现有保存逻辑：裁剪 + BGRA→RGBA + PNG 编码
    capture::save_region(&gpu_pixels, self.width, self.height, rect, path)
}
```

```rust
// 调用处（update 内，按键处理）：
if keys.is_down(Key::Enter) {
    if let Some(rect) = self.selection.bounds() {
        self.save_region(&rect, "output.png", session)?;
        quit();
        return Ok(false);
    }
}
```

### 3.5 与现有 `save_region` 的关系

现有 `save_region(pixels, screen_w, screen_h, rect, path)` 接受 `&[u8]` 像素数据（BGRA 格式），裁剪选区后保存为 PNG。两种调用方式：

- **现有路径**（无标注）：`capture::save_region(&self.pixels, ...)` —— 直接从 GDI 截屏像素读取
- **GPU 路径**（有标注）：`capture_gpu_pixels` → `capture::save_region(&gpu_pixels, ...)`

`save_region` 本身**不需要修改**——它只关心像素数据。

## 4. 边界情况与注意事项

### 4.1 Alpha 预乘

Target bitmap 使用 `D2D1_ALPHA_MODE_PREMULTIPLIED`。Staging bitmap 必须使用相同的 alpha 模式以确保 `CopyFromBitmap` 格式匹配。PNG 编码时，`save_region` 做逐像素 BGRA→RGBA 转换，不涉及 alpha 混合计算，因此预乘与否不影响最终输出。

### 4.2 Map 可能失败

`Map(D2D1_MAP_OPTIONS_READ)` 在以下情况可能失败：
- bitmap 不是用 `CPU_READ` 创建的
- bitmap 当前被用作渲染目标
- GPU 设备丢失

我们的 staging bitmap 用 `CPU_READ | CANNOT_DRAW` 创建，且不是渲染目标，所以前两个条件不触发。设备丢失通过 `Result` 传播，由框架处理。

### 4.3 性能

| 步骤 | 开销 |
|------|------|
| `CreateBitmap`（staging） | ~0.01ms（只分配 GPU 显存） |
| `CopyFromBitmap` | 全屏 8MB × GPU 带宽 ≈ <0.1ms |
| `Map`（READ） | 阻塞等待 GPU 完成 → <1ms |
| 逐行拷贝 + BGRA→RGBA | ~2-5ms（CPU） |
| PNG 编码（Fast 压缩） | ~10-50ms |
| **合计** | **~15-55ms** |

相比当前方案只有 PNG 编码的开销（无 GPU 回读），多了 ~3-5ms。对于截图保存操作，这完全可以接受。

### 4.4 如果 GPU 回读失败

保留 GDI 像素作为 fallback：

```rust
fn save_region(&self, rect: &Rect, path: &str, session: &DrawingSession) -> Result<()> {
    // 尝试 GPU 回读（包含标注）
    if let Ok(ctx) = session.raw().cast::<ID2D1DeviceContext>() {
        if let Ok(gpu_pixels) = capture::capture_gpu_pixels(&ctx, ...) {
            return capture::save_region(&gpu_pixels, ...);
        }
    }
    // Fallback：使用原始 GDI 像素（无标注）
    capture::save_region(&self.pixels, ...)
}
```

## 5. 无需变更的部分

- **`Cargo.toml`**：`d2d` 特性已经启用，不需要新依赖
- **`windows-app` 框架**：`app.rs` 不需要任何修改
- **`selection/`**：`Selection` 结构体和绘制逻辑不变
- **`capture::save_region`**：函数签名参数为 `(&[u8], i32, i32, &Rect, &str)`，保持不变
- **帧循环**：仍然是 `begin_draw → update → drop → present`
- **标注工具系统**（未来实现）：只写 D2D 绘制逻辑，不需要 CPU 版本

## 6. 新增/修改文件清单

| 文件 | 变更类型 | 说明 |
|------|----------|------|
| `src/capture.rs` | 新增 ~50 行 | 添加 `capture_gpu_pixels()` 函数 |
| `src/main.rs` | 修改 ~5 行 | `save_region` 增加 `session` 参数，优先使用 GPU 回读 |

## 7. 备选方案：D3D11 Staging Texture

如果未来出现 D2D `Map` 路径的问题，备选方案是 D3D11 staging texture：

```rust
// 从 SwapChain 拿到 backbuffer
let back_buffer: ID3D11Texture2D = chain.raw_swap_chain().GetBuffer(0)?;

// 创建 staging texture
let staging_desc = D3D11_TEXTURE2D_DESC {
    Usage: D3D11_USAGE_STAGING,
    CPUAccessFlags: D3D11_CPU_ACCESS_READ,
    ..back_desc
};
let staging: ID3D11Texture2D = device.d3d_device().CreateTexture2D(&staging_desc, None)?;

// Copy + Map
d3d_context.CopyResource(&staging, &back_buffer)?;
let mapped = d3d_context.Map(&staging, 0, D3D11_MAP_READ, 0)?;
// 读取 mapped.pData
d3d_context.Unmap(&staging, 0);
```

需要添加 `d3d11`/`d3dcommon` 特性到 Cargo.toml。当前优先使用 D2D 方案，此路径仅作为备用。
