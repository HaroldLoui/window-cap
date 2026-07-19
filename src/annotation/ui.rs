use windows_canvas::{Brush, DrawingSession, Rect, Result, RoundedRect, TextAlignment, TextFormat};

pub fn draw_toolbar(session: &DrawingSession, brush: &Brush, rect: Rect) {
    let rounded_rect = RoundedRect::uniform(rect, 5.0);
    session.draw_rounded_rect(&rounded_rect, brush, 2.0);
}

pub fn draw_tool_icon(session: &DrawingSession, brush: &Brush, rect: Rect, label: &str) -> Result<()> {
    let rounded_rect = RoundedRect::uniform(rect, 2.0);
    session.draw_rounded_rect(&rounded_rect, brush, 2.0);

    let format = TextFormat::new("Segoe UI", 24.0)?.with_alignment(TextAlignment::Center);
    session.draw_text(label, &format, &rect, brush);

    Ok(())
}
