use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::{
    app::{App, EditorMode, Focus},
    syntax,
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
            let marker = if entry.is_updated { "● " } else { "  " };
            let marker_style = if entry.is_updated {
                Style::default()
                    .fg(theme.accent_warn)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.muted)
            };
            let style = if entry.is_updated {
                Style::default()
                    .fg(theme.accent_warn)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };
            ListItem::new(Line::from(vec![
                Span::raw(indent),
                Span::styled(marker, marker_style),
                Span::styled(entry.display.clone(), style),
            ]))
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

    let visible_editor_lines = app
        .editor_lines
        .iter()
        .filter(|line| diff_line_visible(line, app.editor_mode))
        .collect::<Vec<_>>();

    let editor_height = middle[0].height.saturating_sub(2) as usize;
    let max_editor_scroll = visible_editor_lines
        .len()
        .saturating_sub(editor_height.max(1));
    app.editor_scroll = app.editor_scroll.min(max_editor_scroll);

    let editor_text = visible_editor_lines
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

    let right_pane_header = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {} ", app.right_pane_mode.pane_title()),
            codex_header_style,
        ),
        Span::styled(
            format!("live session  [{0}]  Shift+Tab theme", theme.name),
            Style::default().fg(theme.muted).bg(theme.pane_bg),
        ),
    ]))
    .style(Style::default().bg(theme.pane_bg));
    frame.render_widget(right_pane_header, codex_chunks[0]);

    frame.render_widget(
        Block::default().style(Style::default().bg(theme.pane_bg)),
        codex_chunks[1],
    );

    let codex_output_lines = if let Some(session) = app.right_pane_session.as_mut() {
        let width = codex_chunks[1].width.max(1);
        let height = codex_chunks[1].height.max(1);
        let _ = session.resize(width, height);
        session.snapshot_lines(height, width, theme)
    } else {
        vec![Line::styled(
            app.right_pane_status.as_str(),
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
        Span::raw("  "),
        Span::styled(
            " Ctrl+R ",
            Style::default()
                .fg(theme.title_focus_fg)
                .bg(theme.border_focus)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("remote", Style::default().fg(theme.muted).bg(theme.pane_bg)),
    ]))
    .style(Style::default().bg(theme.pane_bg));
    frame.render_widget(codex_footer, codex_chunks[2]);

    if app.show_remote_qr {
        draw_remote_overlay(frame, app, theme);
    }
}

fn editor_line<'a>(line: &'a str, mode: EditorMode, theme: crate::theme::Theme) -> Line<'a> {
    if mode == EditorMode::Diff {
        let style = if line.starts_with('+') && !line.starts_with("+++") {
            Style::default()
                .fg(Color::Rgb(46, 160, 67))
                .bg(diff_add_bg(theme))
        } else if line.starts_with('-') && !line.starts_with("---") {
            Style::default()
                .fg(Color::Rgb(248, 81, 73))
                .bg(diff_remove_bg(theme))
        } else {
            Style::default().fg(theme.text)
        };
        Line::styled(line.to_string(), style)
    } else {
        syntax::highlight_line(line, theme)
    }
}

fn diff_line_visible(line: &str, mode: EditorMode) -> bool {
    if mode != EditorMode::Diff {
        return true;
    }

    !(line.starts_with("@@")
        || line.starts_with("diff --git")
        || line.starts_with("index ")
        || line.starts_with("---")
        || line.starts_with("+++"))
}

fn diff_add_bg(theme: crate::theme::Theme) -> Color {
    if is_light_theme(theme) {
        Color::Rgb(204, 255, 216)
    } else {
        Color::Rgb(20, 61, 39)
    }
}

fn diff_remove_bg(theme: crate::theme::Theme) -> Color {
    if is_light_theme(theme) {
        Color::Rgb(255, 216, 214)
    } else {
        Color::Rgb(73, 27, 31)
    }
}

fn is_light_theme(theme: crate::theme::Theme) -> bool {
    let ratatui::style::Color::Rgb(r, g, b) = theme.pane_bg else {
        return false;
    };
    (u16::from(r) + u16::from(g) + u16::from(b)) > 500
}

fn draw_remote_overlay(frame: &mut Frame, app: &App, theme: crate::theme::Theme) {
    let Some(remote) = app.remote_share.as_ref() else {
        return;
    };

    let area = centered_rect(frame.area(), 76, 84);
    let body = remote
        .qr_lines()
        .iter()
        .map(|line| Line::styled(line.clone(), Style::default().fg(theme.text)))
        .chain([
            Line::from(""),
            Line::styled(
                "Scan to open the ngrok URL on your phone.",
                Style::default().fg(theme.text),
            ),
            Line::styled(
                remote.url().to_string(),
                Style::default().fg(theme.border_focus),
            ),
            Line::styled(
                format!("QR SVG: {}", remote.qr_svg_path().display()),
                Style::default().fg(theme.muted),
            ),
            Line::styled(
                "Phone actions: Enter, y, n, Ctrl+C.",
                Style::default().fg(theme.muted),
            ),
            Line::styled(
                "Esc closes this overlay. Ctrl+R stops sharing.",
                Style::default().fg(theme.muted),
            ),
        ])
        .collect::<Vec<_>>();

    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(body)
            .block(pane_block("Remote Share", true, theme))
            .wrap(Wrap { trim: false })
            .style(Style::default().bg(theme.pane_bg).fg(theme.text)),
        area,
    );
}

fn centered_rect(area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .split(vertical[1])[1]
}
