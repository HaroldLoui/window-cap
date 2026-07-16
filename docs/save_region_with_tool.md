# 保存选区（含标注）方案调研

> 背景：选区交互已完成，底图冻帧已实现，现在要将选区内容（底图 + 标注）保存为 PNG。
> 核心难点：标注工具会越来越复杂（矩形、线段、箭头、文字、马赛克……），方案必须考虑扩展性。

## 1. 当前架构

```
启动:
  GDI BitBlt 截全屏 → pixels: Vec<u8> (BGRA)

每帧渲染 (D2D):
  clear(TRANSPARENT)
  → draw_background (pixels → GPU bitmap → DrawBitmap)
  → selection.draw (遮罩 + 边框 + 手柄)
  → [标注工具各自 draw]  ← 未来添加

保存 (CPU):
  pixels → 裁剪选区 → BGRA→RGBA → PNG 编码
```

两条独立路径：
- **D2D 路径**：用户看到的画面（底图 + 遮罩 + 标注）
- **CPU 路径**：保存到文件的内容（只有底图裁剪，无标注）

问题：保存出来的 PNG 没有标注。

## 2. 方案 A：CPU 重绘标注

### 思路

每种标注写两套绘制逻辑：
- D2D 版本（`session.fill_rect` / `session.draw_line` / `session.draw_text` …）
- CPU 版本（直接操作 `rgba: Vec<u8>` 像素）

保存时在 `save_region` 中，裁剪出选区 `rgba` 后，遍历所有标注的 CPU 绘制方法涂上去。

### 优劣

| 维度 | 评价 |
|------|------|
| 性能 | ✅ 低开销，纯 CPU 像素操作，几 ms |
| 扩展性 | ❌ 每种标注写两套，复杂标注（文字、抗锯齿、渐变）CPU 实现极痛苦 |
| 一致性 | ⚠️ 两套实现可能出现像素级不一致（D2D 抗锯齿 vs CPU 手写） |
| 维护成本 | ❌ 标注越多越不可维护 |

### 结论

适合标注种类少且简单的阶段。长期不可持续。

## 3. 方案 C：从 GPU 读回渲染结果（直接 Map back buffer）

### 思路

标注只写一套 D2D 绘制逻辑。保存时从 swap chain 的 back buffer 读回像素，裁剪选区保存。

```
D2D 画完底图 + 遮罩 + 标注 → back buffer = 完整画面
  ↓
ID2D1Bitmap1::Map(D2D1_MAP_OPTIONS_READ) → 拿到像素
  ↓
裁剪选区 → PNG
```

### 调研结果：❌ 不可行

微软文档明确限制：

> `D2D1_BITMAP_OPTIONS_CPU_READ` requires `D2D1_BITMAP_OPTIONS_CANNOT_DRAW`
> and **cannot be combined with any other flags**.

swap chain 的 target bitmap 创建时用的是：

```
bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW
```

`Map` 要求 `CPU_READ` flag，而 `CPU_READ` 不能和 `TARGET` 组合。所以 back buffer **无法直接 Map**。

## 4. 方案 C'：离屏拷贝 + Map

### 思路

不能直接读 back buffer，但可以创建一张可读的离屏 bitmap，把渲染结果拷过去再读：

```
1. 创建离屏 bitmap: bitmapOptions = CPU_READ | CANNOT_DRAW（同尺寸）
2. D2D 画完底图 + 遮罩 + 标注，EndDraw 完成
3. 离屏 bitmap.CopyFromBitmap(target bitmap)  ← GPU→GPU 拷贝
4. 离屏 bitmap.Map(READ) → 拿到像素           ← GPU→CPU 回读
5. 裁剪选区 → PNG
6. Unmap
```

### 优劣

| 维度 | 评价 |
|------|------|
| 性能 | ⚠️ 多一次 GPU 拷贝 + GPU→CPU 回读，全屏 8MB 传输约几~十几 ms |
| 扩展性 | ✅ 标注只写一套 D2D 逻辑，所见即所得 |
| 一致性 | ✅ 保存 = 屏幕所见，零不一致风险 |
| 实现复杂度 | ⚠️ 需要通过 cast 调 windows crate 的 `CopyFromBitmap` 和 `Map`，绕过 windows-canvas 的 pub(crate) 限制（同 CreateBitmap 的 cast 方案） |
| 时机约束 | ⚠️ CopyFromBitmap 必须在 EndDraw 之后、Present 之前；但当前框架 DrawingSession 的 Drop 触发 EndDraw，之后 session 已失效，需要调整框架或另开 session |

### 时机问题（关键难点）

当前框架的帧循环：

```rust
let session = chain.begin_draw()?;      // BeginDraw
app.update(&ctx, &session)?;            // 用户绘制
drop(session);                           // EndDraw（Drop impl）
chain.present()?;                       // Present
```

`CopyFromBitmap` 需要在 `EndDraw` 之后调用，但 `EndDraw` 是 `DrawingSession::drop` 触发的，drop 之后 `session.raw()` 就不能用了。

可能的解决方式：
- 在 `update` 返回前就做拷贝（EndDraw 之前用 `CopyFromBitmap`，但此时 draw 还没结束）
- 框架层增加一个 `before_present` 回调
- 自己拿到 `ID2D1DeviceContext` 引用，绕过 `DrawingSession` 的生命周期

这个问题需要进一步验证。

## 5. 方案 A + Trait 规范（折中方案）

### 思路

承认双绘制路径的现实，但用 trait 规范接口，让每种标注自己负责两套实现：

```rust
pub trait Annotation {
    /// D2D 绘制（屏幕显示）
    fn draw_d2d(&self, session: &DrawingSession);

    /// CPU 绘制（保存到 rgba）
    fn draw_cpu(&self, rgba: &mut [u8], region_w: i32, region_h: i32, offset_x: i32, offset_y: i32);
}
```

### 优劣

| 维度 | 评价 |
|------|------|
| 性能 | ✅ 低开销 |
| 扩展性 | ⚠️ 新增标注要写两套，但有 trait 规范，编译器会提醒 |
| 一致性 | ⚠️ 仍有两套实现不一致的风险 |
| 实现难度 | ✅ 不涉及 GPU 回读，不需要改框架 |

## 6. 对比总结

| 方案 | 标注写几套 | 保存开销 | 扩展性 | 框架改动 | 一致性 |
|------|-----------|---------|--------|---------|--------|
| A（CPU 重绘） | 两套 | 低 | ❌ 差 | 无 | ⚠️ 风险 |
| C（直接 Map） | 一套 | — | — | — | — |
| C'（离屏拷贝） | 一套 | 中 | ✅ 好 | 需改 | ✅ 一致 |
| A+Trait | 两套 | 低 | ⚠️ 中 | 无 | ⚠️ 风险 |

## 7. 待决定

- 短期先用方案 A 快速出功能，后续再迁移到方案 C'？
- 还是一开始就投方案 C'，避免后期重构？
- 方案 C' 的时机问题（EndDraw 后如何拿到 context）能否解决？

这些问题需要进一步讨论和验证。
