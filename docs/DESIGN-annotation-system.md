# 标注工具系统设计方案

> **日期**：2026-07-14
> **背景**：选区交互已完成，GPU 回读保存已实现。现在要添加 Flameshot 风格的标注工具系统。
> **核心理念**：标注只写一套 D2D 绘制，保存时通过 GPU 回读天然包含标注，所见即所得。

## 1. 交互流程

```
启动 → None（全屏遮罩）
  │
  ├─ 鼠标拖拽选区 → Selecting
  │
  └─ 松手 → Idle（选区确定）
               │
               ⊙ 选区上方出现工具栏
               │
               ├─ 点击工具按钮 → 进入标注模式
               │   ├─ 鼠标事件 → active_tool.handle_events()
               │   ├─ active_tool.draw(session) → 实时预览
               │   └─ 绘制完成（松手/确认）→ 提交 annotation → 切回 Idle
               │
               ├─ 在多选区内点击 → 开始框选新选区（丢弃旧选区）
               │
               ├─ 拖拽手柄 → Resize
               │
               └─ 拖拽选区中间 → Move
```

### 状态机变更

```
当前状态: None → Selecting → Idle → Resize/Move → Idle

新增: Idle 状态下多一层"标注模式"子状态:
  Idle:
    ├─ 点击选区外 → Selecting（重新框选，丢弃旧选区和所有标注）
    ├─ 拖拽手柄 → Resize
    ├─ 拖拽选区 → Move
    ├─ 快捷键(Enter) → 异步保存 → 退出
    ├─ 快捷键(Esc) → 回到 None（清空选区 + 清空标注）
    └─ 点击工具栏按钮 → Idle + active_tool
         │
         └─ 鼠标在选区内绘制 → 提交 annotation → Idle（保留工具栏）
```

## 2. 工具栏设计

### 2.1 视觉效果

```
  ╭───────────────────────────────────────────╮
  │ [S] [R] [C] [A] [F] [T] [B]  | [U] [S] [X]│
  ╰───────────────────────────────────────────╯
   S=选区  R=矩形  C=圆形  A=箭头  F=画笔  T=文字  B=模糊  |  U=撤销  S=保存  X=取消
```

- **背景**：`rgba(30,30,30,0.85)`，圆角 8px，高度 38px
- **按钮**：每个 30×30px，间距 2px，居中排列
- **分隔符**：垂直细线 `rgba(255,255,255,0.2)` 分隔工具区和操作区
- **悬停高亮**：`fill_rect` 半透明白 `rgba(255,255,255,0.12)`，圆角 4px
- **选中高亮**：`fill_rect` 浅蓝 `rgba(0,175,255,0.25)` + 蓝色边框
- **按钮内容**：白色粗体字母或 emoji，20px 居中（`draw_text`）

### 2.2 定位

```rust
pub fn calc_toolbar_rect(selection_bounds: Rect, screen_size: (f32, f32)) -> Rect {
    let tb_w = 350.0;  // 工具栏宽度（固定或自适应按钮数）
    let tb_h = 38.0;   // 工具栏高度
    let gap = 6.0;     // 与选区的间距

    let cx = selection_bounds.left + selection_bounds.width() / 2.0;
    let mut x = cx - tb_w / 2.0;
    let mut y = selection_bounds.top - tb_h - gap;

    // 如果超出屏幕顶部，放到选区内部上沿
    if y < 0.0 {
        y = selection_bounds.top + gap;
    }

    // 左右不能超出屏幕
    if x < 2.0 { x = 2.0; }
    if x + tb_w > screen_size.0 - 2.0 { x = screen_size.0 - tb_w - 2.0; }

    Rect::from_xywh(x, y, tb_w, tb_h)
}
```

### 2.3 数据结构

```rust
pub struct Toolbar {
    outer_rect: Rect,                 // 工具栏外框
    btn_rect: Rect,                   // 工具按钮区范围
    op_rect: Rect,                    // 操作按钮区范围
    pub buttons: Vec<Button>,
    pub hover_idx: Option<usize>,
    pub active_tool: Option<usize>,   // 当前选中的工具索引
}

pub struct Button {
    pub label: &'static str,          // 显示的字母/emoji，如 "R" "C" "A"
    pub tooltip: &'static str,        // 悬停提示，如 "矩形"
    pub rect: Rect,                   // 按钮在屏幕上的位置
    pub kind: ButtonKind,
}

pub enum ButtonKind {
    Tool(ToolKind),
    Save,
    Cancel,
    Undo,
}

pub enum ToolKind {
    Select,    // 切换回选区模式
    Rect,
    Circle,
    Arrow,
    Freehand,
    Text,
    Blur,
}
```

### 2.4 事件处理

```rust
impl Toolbar {
    /// 处理鼠标事件，返回触发的按钮动作（如果有）
    /// 只在 Idle 状态下调用。
    pub fn handle_mouse(&mut self, events: &[Action]) -> Option<ButtonKind> {
        for event in events {
            match *event {
                Action::MouseMove { pos } => {
                    let prev = self.hover_idx;
                    self.hover_idx = self.hit_test(pos);
                    // 返回 None 但 hover 变了 → 需要重绘
                }
                Action::MouseDown { pos, .. } => {
                    if let Some(idx) = self.hit_test(pos) {
                        return Some(self.buttons[idx].kind.clone());
                    }
                }
                _ => {}
            }
        }
        None
    }
}
```

## 3. 标注（Annotation）系统

### 3.1 接口定义

```rust
/// 已提交的标注（不可变，只负责绘制）
pub trait Annotation {
    /// D2D 绘制到 overlay
    fn draw(&self, session: &DrawingSession) -> Result<()>;
}
```

提交后不可变，只绘制。每个标注保存自己的数据：

```rust
pub struct RectAnnotation {
    pub start: Pos2,    // 选区坐标系（相对选区的左上角）
    pub end: Pos2,
    pub color: ColorF,
    pub stroke_width: f32,
}

impl Annotation for RectAnnotation {
    fn draw(&self, session: &DrawingSession) -> Result<()> {
        // 坐标转换：选区坐标系 → 屏幕坐标系
        // 画矩形边框
    }
}
```

### 3.2 工具（Tool）接口

```rust
/// 交互中的标注工具（有状态）
pub trait AnnotationTool {
    /// 处理鼠标事件
    fn handle_event(&mut self, events: &[Action]);
    /// 绘制实时预览
    fn draw(&mut self, session: &DrawingSession) -> Result<()>;
    /// 提交当前内容，生成不可变的 Annotation
    fn commit(self: Box<Self>) -> Box<dyn Annotation>;
    /// 工具名称
    fn name(&self) -> &str;
}
```

### 3.3 坐标体系

标注工具在**屏幕坐标系**中接收事件、进行绘制。提交时保存的也是屏幕坐标。

但注意：标注应该被限制在选区内吗？

- **限制在选区内**：标注不会画出选区边界，但需要 clip 到选区矩形
- **不限制**：标注可以画到选区外，更自由但可能破坏视觉效果

Flameshot 的做法是**不限制**，但默认标注在选区内。我们初期也可以**不限制**，因为选区只是"截图范围"，标注可以是选区的补充说明（引出线、文字标注等都在选区外）。

## 4. Screenshot 结构体变更

### 4.1 新增字段

```rust
pub struct Screenshot {
    // ... 现有字段 ...
    pub annotations: Vec<Box<dyn Annotation>>,  // 已提交标注
    active_tool: Option<Box<dyn AnnotationTool>>, // 当前工具
    toolbar: Toolbar,                              // 工具栏
    mode: Mode,
}
```

### 4.2 状态机

```rust
enum Mode {
    /// 选区交互（None/Selecting/Idle/Resize/Move）
    Selection,
    /// 标注工具活跃
    Annotating,
}
```

### 4.3 update 改动

```rust
fn update(&mut self, ctx: &Ctx, session: &DrawingSession) -> Result<bool> {
    // 1. 惰性创建 GPU bitmap（不变）
    self.ensure_bitmap(session)?;

    // 2. 按键处理
    self.handle_keys(ctx.keys(), session)?;

    // 3. 事件分发（根据模式）
    match self.mode {
        Mode::Selection => {
            self.selection.handle_event(ctx.events());
            selection::handles::set_cursor(self.selection.cursor_style());

            // Idle 状态下：工具栏交互
            if self.selection.state() == selection::State::Idle {
                if let Some(action) = self.toolbar.handle_mouse(ctx.events()) {
                    match action {
                        ButtonKind::Tool(kind) => {
                            self.active_tool = Some(create_tool(kind));
                            self.mode = Mode::Annotating;
                        }
                        ButtonKind::Save => {
                            // 触发保存
                            self.do_save(session);
                        }
                        ButtonKind::Cancel => {
                            self.selection.reset();
                            self.annotations.clear();
                        }
                        ButtonKind::Undo => {
                            self.annotations.pop();
                        }
                    }
                }
            }
        }
        Mode::Annotating => {
            if let Some(tool) = &mut self.active_tool {
                tool.handle_event(ctx.events());
                // 工具自身判断是否完成，通过某种方式通知
                if tool.is_done() {
                    self.annotations.push(tool.commit());
                    self.active_tool = None;
                    self.mode = Mode::Selection;
                }
            }
        }
    }

    // 4. 绘制
    session.clear(ColorF::TRANSPARENT);
    self.draw_background(session);
    self.selection.draw_overlay_only(session)?;

    // 已提交标注（始终绘制）
    for a in &self.annotations {
        a.draw(session)?;
    }

    // 当前工具预览
    if let Some(tool) = &self.active_tool {
        tool.draw(session)?;
    }

    // 边框 + 手柄（遮挡标注，保持交互突出）
    self.selection.draw_border_and_handles(session)?;

    // 工具栏（Idle 状态下显示，在最上层）
    if self.mode == Mode::Selection && self.selection.state() == selection::State::Idle {
        self.toolbar.draw(session)?;
    }

    // 异步保存检查
    self.check_save_done();

    Ok(true)
}
```

### 4.4 绘制层级

```
clear(TRANSPARENT)
  ↓
draw_background              ← 冻帧截图
  ↓
draw_overlay_only            ← 4块遮罩，选区内透明
  ↓
annotations draw             ← 已提交的标注
  ↓
active_tool draw             ← 当前工具的实时预览
  ↓
draw_border_and_handles      ← 选区边框 + 手柄（遮挡标注边缘）
  ↓
toolbar draw                 ← 工具栏（Idle时显示，在最上层）
```

## 5. 文件结构

```
src/
 ├── main.rs
 ├── app.rs                    ← Screenshot 结构体
 ├── capture.rs                ← GDI 截屏 + GPU 回读 + PNG 保存
 ├── brush.rs                  ← BrushState
 ├── utils.rs                  ← normalize
 ├── selection/
 │   ├── mod.rs
 │   ├── selection.rs          ← Selection 选区工具
 │   └── handles.rs            ← 8方向手柄 + 光标
 ├── annotation/
 │   ├── mod.rs                ← pub trait Annotation + pub trait AnnotationTool
 │   ├── toolbar.rs            ← Toolbar 结构体 + 绘制 + 事件
 │   ├── rect.rs               ← RectAnnotation + RectTool
 │   ├── circle.rs
 │   ├── arrow.rs
 │   ├── freehand.rs
 │   ├── text.rs
 │   └── blur.rs
```

## 6. 实现步骤

| 步骤 | 内容 | 产出 |
|------|------|------|
| 1 | 创建 `annotation/mod.rs`，定义 `Annotation` / `AnnotationTool` trait | 接口稳定 |
| 2 | 实现 `Toolbar` 结构体 + 绘制 + 命中检测 | 工具栏可见可点 |
| 3 | `Screenshot` 集成工具栏 + 模式切换 | 工具栏→标注→提交链路走通 |
| 4 | 实现 `RectTool` / `RectAnnotation` | 第一个标注工具可用 |
| 5 | 实现 `CircleTool` | 第二个工具 |
| 6 | 实现 `ArrowTool` | 第三个工具 |
| 7 | 实现 Undo / Save / Cancel 操作 | 完成标注流程闭环 |

## 7. 未解决问题（待讨论）

1. **标注坐标体系**：标注用屏幕坐标还是选区相对坐标？如果选区是 Resize/Move，标注会跟随移动吗？
   - 建议：直接用屏幕坐标。选区只是截图范围参考，标注独立。
2. **工具完成判定**：矩形/圆形在 MouseUp 时自动提交？Enter 确认提交？两种都支持？
   - 建议：MouseUp 自动提交，简单直接。
3. **文字工具**：需要 IME 输入支持，单独走。
   - 建议：一期先不做文字工具，放到二期。
4. **撤销范围**：撤销只撤销标注，还是也撤销选区状态？
   - 建议：只撤销最后一个标注。
5. **取消操作**：取消清空所有标注 + 选区回到 None。
