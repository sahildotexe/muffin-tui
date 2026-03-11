use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, Paragraph, Wrap},
};

use crate::{
    app::{App, EditorMode, Focus},
    theme::{THEMES, pane_block},
};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let root = frame.area();
    let theme = THEMES[app.theme_index];

    frame.render_widget(
        Block::default().style(Style::default().bg(theme.app_bg)),
        root,
    );

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(22),
            Constraint::Percentage(53),
            Constraint::Percentage(25),
        ])
        .split(root);

    let middle = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(9)])
        .split(columns[1]);

    let file_items: Vec<ListItem> = app
        .files
        .iter()
        .map(|entry| {
            let indent = "  ".repeat(entry.depth);
            ListItem::new(Line::from(format!("{indent}{}", entry.display)))
        })
        .collect();

    let file_list = List::new(file_items)
        .block(pane_block("Files", app.focus == Focus::FileTree, theme))
        .style(Style::default().bg(theme.pane_bg).fg(theme.text))
        .highlight_style(
            Style::default()
                .bg(theme.list_highlight_bg)
                .fg(theme.list_highlight_fg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(file_list, columns[0], &mut app.file_state);

    let editor_height = middle[0].height.saturating_sub(2) as usize;
    let max_editor_scroll = app.editor_lines.len().saturating_sub(editor_height.max(1));
    app.editor_scroll = app.editor_scroll.min(max_editor_scroll);

    let editor_text = app
        .editor_lines
        .iter()
        .skip(app.editor_scroll)
        .take(editor_height.max(1))
        .map(|line| editor_line(line, app.editor_mode, theme))
        .collect::<Vec<_>>();

    let editor = Paragraph::new(editor_text)
        .style(Style::default().bg(theme.pane_bg).fg(theme.text))
        .wrap(Wrap { trim: false })
        .block(pane_block(
            &app.editor_title,
            app.focus == Focus::Editor,
            theme,
        ));
    frame.render_widget(editor, middle[0]);

    let terminal_height = middle[1].height.saturating_sub(2) as usize;
    let mut terminal_lines = app
        .terminal_output
        .iter()
        .map(|line| Line::from(line.as_str()))
        .collect::<Vec<_>>();
    terminal_lines.push(Line::from(vec![
        Span::styled("$ ", Style::default().fg(theme.border_focus)),
        Span::styled(app.terminal_input.as_str(), Style::default().fg(theme.text)),
    ]));

    let max_terminal_scroll = terminal_lines.len().saturating_sub(terminal_height.max(1));
    app.terminal_scroll = app.terminal_scroll.min(max_terminal_scroll);
    let end = terminal_lines.len().saturating_sub(app.terminal_scroll);
    let start = end.saturating_sub(terminal_height.max(1));
    let terminal_lines = terminal_lines
        .into_iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect::<Vec<_>>();

    let terminal_pane = Paragraph::new(terminal_lines)
        .style(Style::default().bg(theme.pane_bg).fg(theme.text))
        .wrap(Wrap { trim: false })
        .block(pane_block("Terminal", app.focus == Focus::Terminal, theme));
    frame.render_widget(terminal_pane, middle[1]);

    let codex_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(columns[2]);

    let codex_header_style = if app.focus == Focus::Codex {
        Style::default()
            .fg(theme.title_focus_fg)
            .bg(theme.title_focus_bg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.title).bg(theme.pane_bg)
    };

    let codex_header = Paragraph::new(Line::from(vec![
        Span::styled(" Codex ", codex_header_style),
        Span::styled(
            format!("live session  [{0}]  Shift+Tab theme", theme.name),
            Style::default().fg(theme.muted).bg(theme.pane_bg),
        ),
    ]))
    .style(Style::default().bg(theme.pane_bg));
    frame.render_widget(codex_header, codex_chunks[0]);

    frame.render_widget(
        Block::default().style(Style::default().bg(theme.pane_bg)),
        codex_chunks[1],
    );

    let codex_output_lines = if let Some(codex) = app.codex.as_mut() {
        let width = codex_chunks[1].width.max(1);
        let height = codex_chunks[1].height.max(1);
        let _ = codex.resize(width, height);
        codex.snapshot_lines(height, width, theme)
    } else {
        vec![Line::styled(
            app.codex_status.as_str(),
            Style::default().fg(theme.muted).bg(theme.pane_bg),
        )]
    };

    let codex = Paragraph::new(codex_output_lines).style(Style::default().bg(theme.pane_bg));
    frame.render_widget(codex, codex_chunks[1]);

    let codex_footer = Paragraph::new(Line::from(vec![
        Span::styled(
            " Ctrl+C ",
            Style::default()
                .fg(theme.title_focus_fg)
                .bg(theme.accent_warn)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "interrupt",
            Style::default().fg(theme.muted).bg(theme.pane_bg),
        ),
        Span::raw("  "),
        Span::styled(
            " Tab ",
            Style::default()
                .fg(theme.title_focus_fg)
                .bg(theme.accent_info),
        ),
        Span::styled(
            "leave pane",
            Style::default().fg(theme.muted).bg(theme.pane_bg),
        ),
        Span::raw("  "),
        Span::styled(
            " PgUp/PgDn ",
            Style::default()
                .fg(theme.title_focus_fg)
                .bg(theme.accent_scroll),
        ),
        Span::styled("scroll", Style::default().fg(theme.muted).bg(theme.pane_bg)),
    ]))
    .style(Style::default().bg(theme.pane_bg));
    frame.render_widget(codex_footer, codex_chunks[2]);
}

fn editor_line<'a>(line: &'a str, mode: EditorMode, theme: crate::theme::Theme) -> Line<'a> {
    if mode == EditorMode::Diff {
        let style = if line.starts_with('+') && !line.starts_with("+++") {
            Style::default().fg(Color::Rgb(108, 214, 141))
        } else if line.starts_with('-') && !line.starts_with("---") {
            Style::default().fg(Color::Rgb(255, 120, 117))
        } else if line.starts_with("@@") {
            Style::default()
                .fg(theme.border_focus)
                .add_modifier(Modifier::BOLD)
        } else if line.starts_with("diff --git")
            || line.starts_with("index ")
            || line.starts_with("---")
            || line.starts_with("+++")
        {
            Style::default().fg(theme.muted)
        } else {
            Style::default().fg(theme.text)
        };
        Line::styled(line.to_string(), style)
    } else {
        Line::styled(line.to_string(), Style::default().fg(theme.text))
    }
}

#[cfg(test)]
mod tests {
    use super::{draw, editor_line};
    use crate::{
        app::{App, EditorMode, Focus},
        theme::THEMES,
    };
    use ratatui::{
        Terminal,
        backend::TestBackend,
        buffer::Buffer,
        layout::Rect,
        widgets::{Paragraph, Widget},
    };

    #[test]
    fn editor_line_styles_diff_markers() {
        let line = editor_line("+added", EditorMode::Diff, THEMES[0]);
        let area = Rect::new(0, 0, 10, 1);
        let mut buffer = Buffer::empty(area);
        Paragraph::new(line).render(area, &mut buffer);

        assert_eq!(
            buffer[(0, 0)].fg,
            ratatui::style::Color::Rgb(108, 214, 141)
        );
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
}
