use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders},
};

#[derive(Copy, Clone)]
pub struct Theme {
    pub name: &'static str,
    pub app_bg: Color,
    pub pane_bg: Color,
    pub border: Color,
    pub border_focus: Color,
    pub title: Color,
    pub title_focus_fg: Color,
    pub title_focus_bg: Color,
    pub text: Color,
    pub muted: Color,
    pub list_highlight_bg: Color,
    pub list_highlight_fg: Color,
    pub codex_cursor: Color,
    pub accent_warn: Color,
    pub accent_info: Color,
    pub accent_scroll: Color,
}

pub const THEMES: [Theme; 3] = [
    Theme {
        name: "Teal Night",
        app_bg: Color::Rgb(8, 11, 15),
        pane_bg: Color::Rgb(11, 14, 18),
        border: Color::Rgb(92, 104, 118),
        border_focus: Color::Rgb(95, 211, 188),
        title: Color::Rgb(180, 188, 196),
        title_focus_fg: Color::Black,
        title_focus_bg: Color::Rgb(95, 211, 188),
        text: Color::Rgb(232, 238, 242),
        muted: Color::Rgb(130, 145, 160),
        list_highlight_bg: Color::Rgb(39, 47, 58),
        list_highlight_fg: Color::Rgb(255, 224, 102),
        codex_cursor: Color::Rgb(95, 211, 188),
        accent_warn: Color::Rgb(241, 196, 15),
        accent_info: Color::Rgb(80, 227, 194),
        accent_scroll: Color::Rgb(137, 180, 250),
    },
    Theme {
        name: "Amber Graphite",
        app_bg: Color::Rgb(18, 15, 12),
        pane_bg: Color::Rgb(24, 21, 17),
        border: Color::Rgb(122, 104, 90),
        border_focus: Color::Rgb(255, 176, 90),
        title: Color::Rgb(205, 190, 174),
        title_focus_fg: Color::Black,
        title_focus_bg: Color::Rgb(255, 176, 90),
        text: Color::Rgb(244, 235, 225),
        muted: Color::Rgb(169, 152, 136),
        list_highlight_bg: Color::Rgb(58, 44, 31),
        list_highlight_fg: Color::Rgb(255, 220, 160),
        codex_cursor: Color::Rgb(255, 176, 90),
        accent_warn: Color::Rgb(255, 213, 79),
        accent_info: Color::Rgb(255, 142, 83),
        accent_scroll: Color::Rgb(186, 230, 126),
    },
    Theme {
        name: "Ice Blue",
        app_bg: Color::Rgb(232, 238, 245),
        pane_bg: Color::Rgb(244, 248, 252),
        border: Color::Rgb(132, 151, 170),
        border_focus: Color::Rgb(50, 127, 191),
        title: Color::Rgb(82, 100, 119),
        title_focus_fg: Color::White,
        title_focus_bg: Color::Rgb(50, 127, 191),
        text: Color::Rgb(20, 35, 48),
        muted: Color::Rgb(96, 116, 136),
        list_highlight_bg: Color::Rgb(205, 223, 240),
        list_highlight_fg: Color::Rgb(16, 57, 95),
        codex_cursor: Color::Rgb(50, 127, 191),
        accent_warn: Color::Rgb(232, 180, 44),
        accent_info: Color::Rgb(95, 168, 211),
        accent_scroll: Color::Rgb(117, 197, 143),
    },
];

pub fn pane_block(title: &str, focused: bool, theme: Theme) -> Block<'static> {
    let border_style = if focused {
        Style::default()
            .fg(theme.border_focus)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.border)
    };

    let title_style = if focused {
        Style::default()
            .fg(theme.title_focus_fg)
            .bg(theme.title_focus_bg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.title).bg(theme.pane_bg)
    };

    Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().bg(theme.pane_bg).fg(theme.text))
        .title(Span::styled(title.to_string(), title_style))
}
