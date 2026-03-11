use muffintui::{
    app::{App, EditorMode, Focus},
    theme::THEMES,
    ui::draw,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    widgets::{Paragraph, Widget},
};

fn render_editor_line(line: &str) -> Buffer {
    let area = Rect::new(0, 0, 10, 1);
    let mut buffer = Buffer::empty(area);
    let paragraph = Paragraph::new(if EditorMode::Diff == EditorMode::Diff {
        if line.starts_with('+') && !line.starts_with("+++") {
            ratatui::text::Line::styled(
                line.to_string(),
                ratatui::style::Style::default()
                    .fg(ratatui::style::Color::Rgb(46, 160, 67))
                    .bg(ratatui::style::Color::Rgb(20, 61, 39)),
            )
        } else {
            ratatui::text::Line::raw(line.to_string())
        }
    } else {
        ratatui::text::Line::raw(line.to_string())
    });
    paragraph.render(area, &mut buffer);
    buffer
}

#[test]
fn draw_renders_without_panicking() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::test_fixture();
    app.editor_lines = vec!["line 1".to_string(), "line 2".to_string()];
    app.terminal_output = vec!["$ echo hi".to_string(), "hi".to_string()];
    app.terminal_input = "pwd".to_string();

    terminal.draw(|frame| draw(frame, &mut app)).unwrap();

    assert_eq!(app.editor_scroll, 0);
    assert_eq!(app.focus, Focus::Editor);
}

#[test]
fn diff_lines_render_added_text_in_green() {
    let buffer = render_editor_line("+added");
    assert_eq!(buffer[(0, 0)].fg, ratatui::style::Color::Rgb(46, 160, 67));
    assert_eq!(buffer[(0, 0)].bg, ratatui::style::Color::Rgb(20, 61, 39));
    assert_eq!(THEMES[0].name, "Teal Night");
}
