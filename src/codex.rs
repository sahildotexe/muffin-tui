use std::{
    io,
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex},
    thread,
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::theme::Theme;

pub struct CodexSession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn io::Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    parser: Arc<Mutex<vt100::Parser>>,
}

impl CodexSession {
    pub fn start(cwd: &Path, cols: u16, rows: u16) -> io::Result<Self> {
        let pty_system = native_pty_system();
        let pty_pair = pty_system
            .openpty(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(io_other)?;

        let mut cmd = CommandBuilder::new("codex");
        cmd.env("TERM", "xterm-256color");
        cmd.cwd(cwd);
        let child = pty_pair.slave.spawn_command(cmd).map_err(io_other)?;
        drop(pty_pair.slave);

        let mut reader = pty_pair.master.try_clone_reader().map_err(io_other)?;
        let writer = pty_pair.master.take_writer().map_err(io_other)?;
        let parser = Arc::new(Mutex::new(vt100::Parser::new(
            rows.max(1),
            cols.max(1),
            20_000,
        )));
        let parser_reader = Arc::clone(&parser);

        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        if let Ok(mut parser) = parser_reader.lock() {
                            parser.process(b"\r\n[codex session ended]\r\n");
                        }
                        break;
                    }
                    Ok(n) => {
                        if let Ok(mut parser) = parser_reader.lock() {
                            parser.process(&buf[..n]);
                        }
                    }
                    Err(_) => {
                        if let Ok(mut parser) = parser_reader.lock() {
                            parser.process(b"\r\n[codex read error]\r\n");
                        }
                        break;
                    }
                }
            }
        });

        Ok(Self {
            master: pty_pair.master,
            writer,
            child,
            parser,
        })
    }

    pub fn send_ctrl_c(&mut self) -> io::Result<()> {
        self.writer.write_all(&[0x03])?;
        self.writer.flush()
    }

    pub fn send_key(&mut self, key: KeyEvent) -> io::Result<()> {
        match key.code {
            KeyCode::Enter => self.writer.write_all(b"\r")?,
            KeyCode::Backspace => self.writer.write_all(&[0x7f])?,
            KeyCode::Left => self.writer.write_all(b"\x1b[D")?,
            KeyCode::Right => self.writer.write_all(b"\x1b[C")?,
            KeyCode::Up => self.writer.write_all(b"\x1b[A")?,
            KeyCode::Down => self.writer.write_all(b"\x1b[B")?,
            KeyCode::PageUp => self.writer.write_all(b"\x1b[5~")?,
            KeyCode::PageDown => self.writer.write_all(b"\x1b[6~")?,
            KeyCode::Home => self.writer.write_all(b"\x1b[H")?,
            KeyCode::End => self.writer.write_all(b"\x1b[F")?,
            KeyCode::Tab => self.writer.write_all(b"\t")?,
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    let byte = match c {
                        'a'..='z' => Some((c as u8) - b'a' + 1),
                        'A'..='Z' => Some((c as u8) - b'A' + 1),
                        _ => None,
                    };
                    if let Some(byte) = byte {
                        self.writer.write_all(&[byte])?;
                    }
                    return self.writer.flush();
                }
                let mut encoded = [0u8; 4];
                let s = c.encode_utf8(&mut encoded);
                self.writer.write_all(s.as_bytes())?;
            }
            _ => {}
        }

        self.writer.flush()
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> io::Result<()> {
        self.master
            .resize(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(io_other)?;

        if let Ok(mut parser) = self.parser.lock() {
            parser.set_size(rows.max(1), cols.max(1));
        }

        Ok(())
    }

    pub fn snapshot_lines(&self, rows: u16, cols: u16, theme: Theme) -> Vec<Line<'static>> {
        if let Ok(parser) = self.parser.lock() {
            let screen = parser.screen();
            let (screen_rows, screen_cols) = screen.size();
            let visible_rows = rows.min(screen_rows);
            let visible_cols = cols.min(screen_cols);
            let row_offset = screen_rows.saturating_sub(visible_rows);
            let (cursor_row, cursor_col) = screen.cursor_position();
            let show_cursor = !screen.hide_cursor();
            let mut out = Vec::with_capacity(visible_rows as usize);

            for row in row_offset..screen_rows {
                let mut spans = Vec::new();
                let mut current_style: Option<Style> = None;
                let mut current_text = String::new();

                for col in 0..visible_cols {
                    let Some(cell) = screen.cell(row, col) else {
                        continue;
                    };

                    if cell.is_wide_continuation() {
                        continue;
                    }

                    let mut style = style_from_vt100_cell(cell, theme).bg(theme.pane_bg);
                    if show_cursor && row == cursor_row && col == cursor_col {
                        style = style
                            .fg(theme.pane_bg)
                            .bg(theme.codex_cursor)
                            .add_modifier(Modifier::BOLD);
                    }

                    let text = if cell.has_contents() {
                        cell.contents()
                    } else {
                        " ".to_string()
                    };

                    if current_style == Some(style) {
                        current_text.push_str(&text);
                    } else {
                        if let Some(prev_style) = current_style {
                            spans.push(Span::styled(std::mem::take(&mut current_text), prev_style));
                        }
                        current_style = Some(style);
                        current_text.push_str(&text);
                    }
                }

                if let Some(style) = current_style {
                    spans.push(Span::styled(current_text, style));
                } else {
                    spans.push(Span::styled(
                        " ".repeat(visible_cols as usize),
                        Style::default().bg(theme.pane_bg),
                    ));
                }

                out.push(Line::from(spans));
            }

            if out.is_empty() {
                out.push(Line::styled("", Style::default().bg(theme.pane_bg)));
            }

            out
        } else {
            vec![Line::styled(
                "[codex output unavailable]",
                Style::default().fg(theme.muted).bg(theme.pane_bg),
            )]
        }
    }
}

impl Drop for CodexSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn style_from_vt100_cell(cell: &vt100::Cell, theme: Theme) -> Style {
    let mut style = Style::default()
        .fg(color_from_vt100(cell.fgcolor(), false, theme))
        .bg(color_from_vt100(cell.bgcolor(), true, theme));

    if cell.bold() {
        style = style.add_modifier(Modifier::BOLD);
    }
    if cell.italic() {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if cell.underline() {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    if cell.inverse() {
        style = style.fg(color_from_vt100(cell.bgcolor(), false, theme));
        style = style.bg(color_from_vt100(cell.fgcolor(), true, theme));
    }

    style
}

fn color_from_vt100(color: vt100::Color, background: bool, theme: Theme) -> Color {
    match color {
        vt100::Color::Default => {
            if background {
                theme.pane_bg
            } else {
                theme.text
            }
        }
        vt100::Color::Idx(idx) => ansi_256_to_ratatui(idx),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

fn ansi_256_to_ratatui(idx: u8) -> Color {
    const ANSI_BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (92, 92, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];

    match idx {
        0..=15 => {
            let (r, g, b) = ANSI_BASE[idx as usize];
            Color::Rgb(r, g, b)
        }
        16..=231 => {
            let offset = idx - 16;
            let r = offset / 36;
            let g = (offset % 36) / 6;
            let b = offset % 6;
            let scale = |n: u8| if n == 0 { 0 } else { 55 + (n * 40) };
            Color::Rgb(scale(r), scale(g), scale(b))
        }
        232..=255 => {
            let shade = 8 + ((idx - 232) * 10);
            Color::Rgb(shade, shade, shade)
        }
    }
}

fn io_other<E: ToString>(err: E) -> io::Error {
    io::Error::other(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{ansi_256_to_ratatui, color_from_vt100, io_other};
    use crate::theme::THEMES;
    use ratatui::style::Color;
    use std::io::ErrorKind;

    #[test]
    fn translates_default_vt100_colors_from_theme() {
        let theme = THEMES[0];
        assert_eq!(
            color_from_vt100(vt100::Color::Default, false, theme),
            theme.text
        );
        assert_eq!(
            color_from_vt100(vt100::Color::Default, true, theme),
            theme.pane_bg
        );
    }

    #[test]
    fn translates_ansi_palette_entries() {
        assert_eq!(ansi_256_to_ratatui(0), Color::Rgb(0, 0, 0));
        assert_eq!(ansi_256_to_ratatui(15), Color::Rgb(255, 255, 255));
        assert_eq!(ansi_256_to_ratatui(232), Color::Rgb(8, 8, 8));
    }

    #[test]
    fn wraps_custom_error_as_io_error() {
        let err = io_other("boom");
        assert_eq!(err.kind(), ErrorKind::Other);
        assert_eq!(err.to_string(), "boom");
    }
}
