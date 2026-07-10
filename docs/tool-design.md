# Tool 系统设计文档

## 1. 问题

当前所有事件处理和绘制逻辑都写在 `MyApp::update` 里。当工具数量增加（画线、自由绘制、选框、标注……），update 会膨胀为一个巨型 match，职责混杂：

```rust
// 工具 1：只关心起点和终点
// 工具 2：需要起点到终点间所有中间点
// 工具 3：只需要单次点击位置
// ...全部堆在一个 update 里
```

## 2. 设计目标

| 目标 | 说明 |
|------|------|
| 工具自包含 | 每个工具独立管理自己的状态和行为 |
| 开闭原则 | 新增工具不修改 `MyApp::update` |
| 框架零改动 | 只利用现有 `ctx.events()` + `ctx.mouse()` 原语 |
| 可组合 | 工具之间不互相影响 |

## 3. Tool Trait

```rust
use windows_app::{Action, MouseState};
use windows_canvas::{Brush, ColorF, DrawingSession};

/// 绘制工具 trait — 每个工具自包含事件处理和绘制逻辑
pub trait Tool {
    /// 处理输入事件
    ///
    /// - `events`: 本帧的瞬时事件列表（MouseDown/Up/Move 等）
    /// - `mouse`: 当前鼠标状态快照（坐标 + 按钮）
    fn handle_input(&mut self, events: &[Action], mouse: &MouseState);

    /// 绘制工具的视觉内容
    ///
    /// - `session`: 绘图会话（框架已 clear，工具只需画自己的内容）
    ///
    /// **`&mut self`** — 因为 brush 采用惰性创建（首次 draw 时通过 session 初始化，
    /// 之后复用同一实例，换色时调用 `brush.set_color()` 而非重建）。
    fn draw(&mut self, session: &DrawingSession) -> Result<()>;

    /// 工具名称（用于 UI 显示、调试等）
    fn name(&self) -> &str;

    /// 更换描边宽度（默认空操作，描边类工具重写）
    fn set_stroke_width(&mut self, _width: f32) {}

    /// 更换颜色（默认空操作，需要的工具重写）
    ///
    /// 重写时应同时更新内部 brush 的颜色：`self.brush.set_color(color)`
    fn set_color(&mut self, _color: ColorF) {}
}
```

**设计要点：**

- `handle_input` 同时接收 `events`（离散事件）和 `mouse`（连续状态）——工具按需选用
- `draw` 接收 `&mut self`，因为 brush 采用**惰性创建**（首次 `draw` 时通过 session 初始化，之后复用；换色用 `brush.set_color()` 而非重建）
- `set_stroke_width` / `set_color` 带默认空实现——只有实际需要的工具才重写，外部可统一调用
- 每个工具持有自己的 `Brush`（惰性）和 `stroke_width`，互不干扰

## 4. MyApp 集成

```rust
struct MyApp {
    fullscreen: Rect,
    tool: Box<dyn Tool>,
}

impl App for MyApp {
    fn update(&mut self, ctx: &Ctx, session: &DrawingSession) -> Result<bool> {
        if ctx.keys().is_down(Event::KEY_ESC) {
            quit();
            return Ok(false);
        }

        // ── 输入分发给当前工具 ──
        self.tool.handle_input(ctx.events(), &ctx.mouse());

        // ── 绘制 ──
        session.clear(ColorF::TRANSPARENT);

        // 背景覆盖层
        let brush = session.create_solid_brush(ColorF::new(0.0, 0.0, 0.0, 0.3))?;
        session.fill_rect(&self.fullscreen, &brush);

        // 工具绘制自己的内容（工具内部持有并复用自己的 brush）
        self.tool.draw(session)?;

        Ok(true)
    }
}
```

update 变成**薄调度层**——只管"接事件 → 转给工具" + "画背景 → 让工具画自己"。

## 5. 工具实现示例

### 5.1 画线工具（起点 + 终点）

```rust
pub struct LineTool {
    start: Option<(i32, i32)>,
    end: Option<(i32, i32)>,
    paint: BrushState,
}

impl LineTool {
    pub fn new(color: ColorF) -> Self {
        Self { start: None, end: None, paint: BrushState::new(color) }
    }
}

impl Tool for LineTool {
    fn handle_input(&mut self, events: &[Action], _mouse: &MouseState) {
        for e in events {
            match e {
                Action::MouseDown { button: MouseButton::Left, x, y } => {
                    self.start = Some((*x, *y));
                    self.end = Some((*x, *y));
                }
                Action::MouseUp { button: MouseButton::Left, x, y } => {
                    self.end = Some((*x, *y));
                }
                _ => {}
            }
        }
    }

    fn draw(&mut self, session: &DrawingSession) -> Result<()> {
        let brush = self.paint.brush(session)?;

        if let (Some((sx, sy)), Some((ex, ey))) = (self.start, self.end) {
            session.draw_line(
                Vector2::new(sx as f32, sy as f32),
                Vector2::new(ex as f32, ey as f32),
                brush,
                self.paint.stroke_width,
            );
        }
        Ok(())
    }

    fn name(&self) -> &str { "Line" }

    fn set_stroke_width(&mut self, width: f32) {
        self.paint.set_stroke_width(width);
    }

    fn set_color(&mut self, color: ColorF) {
        self.paint.set_color(color);
    }
}
```

### 5.2 自由绘制工具（记录所有中间点）

```rust
pub struct FreehandTool {
    points: Vec<(i32, i32)>,
    drawing: bool,
    paint: BrushState,
}

impl FreehandTool {
    pub fn new(color: ColorF) -> Self {
        Self { points: Vec::new(), drawing: false, paint: BrushState::new(color) }
    }
}

impl Tool for FreehandTool {
    fn handle_input(&mut self, events: &[Action], mouse: &MouseState) {
        // events 处理边沿
        for e in events {
            match e {
                Action::MouseDown { button: MouseButton::Left, x, y } => {
                    self.drawing = true;
                    self.points.clear();
                    self.points.push((*x, *y));
                }
                Action::MouseUp { button: MouseButton::Left, .. } => {
                    self.drawing = false;
                }
                _ => {}
            }
        }
        // mouse 处理持续（拖拽中间帧）
        if self.drawing {
            self.points.push((mouse.x, mouse.y));
        }
    }

    fn draw(&mut self, session: &DrawingSession) -> Result<()> {
        if self.points.len() < 2 { return Ok(()); }
        let brush = self.paint.brush(session)?;

        // 用 self.points 画线...
        // session.draw_path()

        Ok(())
    }

    fn name(&self) -> &str { "Freehand" }

    fn set_stroke_width(&mut self, width: f32) {
        self.paint.set_stroke_width(width);
    }

    fn set_color(&mut self, color: ColorF) {
        self.paint.set_color(color);
    }
}
```

### 5.3 单次点击工具（纯填充，无描边，如数字序号（填充圆+数字）、emoji等）

```rust
pub struct EmojiTool {
    point: Option<(i32, i32)>,
    emoji: Option<String>,
    paint: BrushState,
}

impl EmojiTool {
    pub fn new(color: ColorF) -> Self {
        Self { point: None, emoji: None, paint: BrushState::new(color) }
    }
}

impl Tool for EmojiTool {
    fn handle_input(&mut self, events: &[Action], _mouse: &MouseState) {
        for e in events {
            if let Action::MouseDown { button: MouseButton::Left, x, y } = e {
                self.point = Some((*x, *y));
            }
        }
    }

    fn draw(&mut self, session: &DrawingSession) -> Result<()> {
        let brush = self.paint.brush(session)?;

        // 绘制emoji
        // session.draw_...

        Ok(())
    }

    fn name(&self) -> &str { "Emoji" }

    fn set_stroke_width(&mut self, width: f32) {
        self.paint.set_stroke_width(width);
    }

    fn set_color(&mut self, color: ColorF) {
        self.paint.set_color(color);
    }
}
```

## 6. 工具切换

```rust
impl MyApp {
    fn switch_tool(&mut self, tool: Box<dyn Tool>) {
        println!("switched to: {}", tool.name());
        self.tool = tool;
    }
}

// 在 update 中用键盘快捷键切换
if ctx.keys().is_down(Event::KEY_1) {
    self.switch_tool(Box::new(LineTool::new(ColorF::new(1.0, 1.0, 0.0, 1.0))));
}
if ctx.keys().is_down(Event::KEY_2) {
    self.switch_tool(Box::new(FreehandTool::new(ColorF::new(0.0, 1.0, 0.0, 1.0))));
}
if ctx.keys().is_down(Event::KEY_3) {
    self.switch_tool(Box::new(EmojiTool::new(ColorF::new(1.0, 0.0, 0.0, 1.0))));
}
```

## 7. 扩展方式

### 新增工具

只需：
1. 新建一个 struct 实现 `Tool` trait
2. 在工具切换逻辑中注册

**不修改** `MyApp::update`、`App` trait、框架代码。

### 工具需要更多上下文

如果某个工具需要访问屏幕尺寸、其他工具的结果等，有两种方式：

**方式 A：`draw` 前缓存中间数据到 `MyApp`**

```rust
// MyApp::update 中
self.tool.handle_input(ctx.events(), &ctx.mouse());

// 如果工具需要知道屏幕尺寸
if let Some(rect_tool) = self.tool.as_any_mut().downcast_mut::<RectTool>() {
    rect_tool.set_bounds(&self.fullscreen);
}
```

**方式 B：扩展 `Tool` trait 的上下文参数**

```rust
/// 工具上下文（按需扩展）
pub struct ToolCtx {
    pub screen: Rect,
    pub mouse: MouseState,
    pub keys: KeyState,
}

pub trait Tool {
    fn handle_input(&mut self, ctx: &ToolCtx, events: &[Action]);
    fn draw(&mut self, session: &DrawingSession) -> Result<()>;
    fn name(&self) -> &str;
}
```

方式 A 适合少量特例，方式 B 适合工具普遍需要额外上下文的场景。**建议先用 A，出现三个以上工具需要同样信息时再升级到 B。**

### 工具需要历史数据

有些工具（如撤销/重做）需要维护操作历史。这属于工具内部状态，在 trait 实现里自行管理即可，不需要框架介入：

```rust
pub struct HistoryTool {
    history: Vec<Action>,   // 操作历史
    undone: Vec<Action>,    // 撤销栈
    // ...
}

impl Tool for HistoryTool {
    fn handle_input(&mut self, events: &[Action], mouse: &MouseState) {
        for e in events {
            // Ctrl+Z → 撤销
            // Ctrl+Y → 重做
            // 正常输入 → 记录到 history
        }
    }
    // ...
}
```

## 8. Brush 管理策略

### 为什么惰性创建？

`Brush` 通过 `DrawingSession::create_solid_brush` 创建，底层是 GPU 资源（`ID2D1SolidColorBrush`），
需要 session 引用。而 session 只在 `draw` 时才拿到，所以 brush 无法在 `new()` 时创建。

### BrushState — 消除重复

每个工具都需要 `color`、`stroke_width`、`brush: Option<Brush>` 三个字段，
以及惰性初始化、`set_color`、`set_stroke_width` 的样板代码。
提取为通用辅助 struct：

```rust
/// 工具的画笔状态 — 管理 brush 惰性创建、换色、描边宽度
pub struct BrushState {
    pub color: ColorF,
    pub stroke_width: f32,
    brush: Option<Brush>,
}

impl BrushState {
    pub fn new(color: ColorF) -> Self {
        Self { color, stroke_width: 2.0, brush: None }
    }

    /// 获取或创建 brush（惰性初始化）
    pub fn brush(&mut self, session: &DrawingSession) -> Result<&Brush> {
        if self.brush.is_none() {
            self.brush = Some(session.create_solid_brush(self.color)?);
        }
        Ok(self.brush.as_ref().unwrap())
    }

    pub fn set_color(&mut self, color: ColorF) {
        self.color = color;
        if let Some(ref brush) = self.brush {
            brush.set_color(color);
        }
    }

    pub fn set_stroke_width(&mut self, width: f32) {
        self.stroke_width = width;
    }
}
```

### 多 brush 场景

如果工具需要多种颜色（如选框的填充 + 边框），持有多个 `BrushState` 即可：

```rust
pub struct RectTool {
    fill: BrushState,
    stroke: BrushState,
    // ...
}
```

## 9. 文件结构

```
src/
 ├── main.rs
 └── tools/
      ├── mod.rs           ← pub trait Tool + re-export
      ├── brush_state.rs   ← BrushState（画笔状态管理）
      ├── line.rs          ← LineTool
      ├── freehand.rs      ← FreehandTool
      └── emoji.rs         ← EmojiTool
```

`tools/mod.rs`:

```rust
mod brush_state;
mod line;
mod freehand;
mod emoji;

pub use brush_state::BrushState;
pub use line::LineTool;
pub use freehand::FreehandTool;
pub use emoji::EmojiTool;

use windows_app::{Action, MouseState};
use windows_canvas::{ColorF, DrawingSession, Result};

/// 绘制工具 trait
pub trait Tool {
    fn handle_input(&mut self, events: &[Action], mouse: &MouseState);
    fn draw(&mut self, session: &DrawingSession) -> Result<()>;
    fn name(&self) -> &str;

    /// 更换描边宽度（默认空操作，描边类工具重写）
    fn set_stroke_width(&mut self, _width: f32) {}

    /// 更换颜色（默认空操作，需要的工具重写）
    fn set_color(&mut self, _color: ColorF) {}
}
```
