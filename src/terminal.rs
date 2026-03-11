use std::process::Command;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn update_input_buffer(buffer: &mut String, key: KeyEvent) -> Option<String> {
    match key.code {
        KeyCode::Enter => {
            let submitted = buffer.clone();
            buffer.clear();
            Some(submitted)
        }
        KeyCode::Backspace => {
            buffer.pop();
            None
        }
        KeyCode::Char(c) => {
            if !key.modifiers.contains(KeyModifiers::CONTROL) {
                buffer.push(c);
            }
            None
        }
        _ => None,
    }
}

pub fn run_shell_command(cmd: &str) -> Vec<String> {
    match Command::new("sh").arg("-lc").arg(cmd).output() {
        Ok(output) => {
            let mut lines = Vec::new();
            lines.extend(bytes_to_lines(&output.stdout));
            lines.extend(bytes_to_lines(&output.stderr));

            if !output.status.success() {
                lines.push(format!(
                    "[exit status: {}]",
                    output.status.code().unwrap_or(-1)
                ));
            }

            lines
        }
        Err(err) => vec![format!("Failed to run command: {}", err)],
    }
}

pub fn bytes_to_lines(bytes: &[u8]) -> Vec<String> {
    let text = strip_ansi_escape_sequences(&String::from_utf8_lossy(bytes));
    let mut lines: Vec<String> = text.lines().map(ToOwned::to_owned).collect();
    if lines.is_empty() && !text.trim().is_empty() {
        lines.push(text.trim().to_string());
    }
    lines
}

pub fn is_terminal_clear_command(cmd: &str) -> bool {
    matches!(cmd, "clear" | "clear;" | "cls" | "cls;")
}

pub fn handle_scrollback_key(key: KeyEvent, terminal_scroll: &mut usize) -> bool {
    match key.code {
        KeyCode::PageUp => {
            *terminal_scroll = terminal_scroll.saturating_add(8);
            true
        }
        KeyCode::PageDown => {
            *terminal_scroll = terminal_scroll.saturating_sub(8);
            true
        }
        KeyCode::Home => {
            *terminal_scroll = usize::MAX;
            true
        }
        KeyCode::End => {
            *terminal_scroll = 0;
            true
        }
        _ => false,
    }
}

pub fn push_capped_line(lines: &mut Vec<String>, line: String) {
    const MAX_LINES: usize = 500;
    lines.push(line);
    if lines.len() > MAX_LINES {
        let overflow = lines.len() - MAX_LINES;
        lines.drain(0..overflow);
    }
}

fn strip_ansi_escape_sequences(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }

        match chars.next() {
            Some('[') => {
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            Some(']') => loop {
                match chars.next() {
                    Some('\u{7}') | None => break,
                    Some('\u{1b}') => {
                        if chars.next_if_eq(&'\\').is_some() {
                            break;
                        }
                    }
                    Some(_) => {}
                }
            },
            Some(_) | None => {}
        }
    }

    out
}
