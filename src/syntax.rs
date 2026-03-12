use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::theme::Theme;

const KEYWORDS: &[&str] = &[
    "as",
    "async",
    "await",
    "break",
    "const",
    "continue",
    "crate",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "self",
    "Self",
    "static",
    "struct",
    "trait",
    "true",
    "type",
    "use",
    "where",
    "while",
    "yield",
    "class",
    "def",
    "elif",
    "export",
    "from",
    "function",
    "import",
    "interface",
    "package",
    "private",
    "protected",
    "public",
    "switch",
    "var",
    "void",
];

pub fn highlight_line<'a>(line: &'a str, theme: Theme) -> Line<'a> {
    let mut spans = Vec::new();
    let mut plain = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if let Some(comment_start) = line_comment_start(&chars, i) {
            flush_plain(&mut spans, &mut plain, theme);
            let comment: String = chars[comment_start..].iter().collect();
            spans.push(Span::styled(comment, comment_style(theme)));
            i = chars.len();
            continue;
        }

        let ch = chars[i];

        if ch == '"' || ch == '\'' {
            flush_plain(&mut spans, &mut plain, theme);
            let (token, next) = consume_quoted(&chars, i, ch);
            spans.push(Span::styled(token, string_style(theme)));
            i = next;
            continue;
        }

        if ch.is_ascii_digit() {
            flush_plain(&mut spans, &mut plain, theme);
            let (token, next) = consume_number(&chars, i);
            spans.push(Span::styled(token, number_style(theme)));
            i = next;
            continue;
        }

        if is_ident_start(ch) {
            let (token, next) = consume_ident(&chars, i);
            if let Some(style) = classify_ident(&token, theme) {
                flush_plain(&mut spans, &mut plain, theme);
                spans.push(Span::styled(token, style));
            } else {
                plain.push_str(&token);
            }
            i = next;
            continue;
        }

        plain.push(ch);
        i += 1;
    }

    flush_plain(&mut spans, &mut plain, theme);
    Line::from(spans)
}

fn line_comment_start(chars: &[char], i: usize) -> Option<usize> {
    if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
        return Some(i);
    }

    let is_hash_comment = chars[i] == '#' && chars[..i].iter().all(|ch| ch.is_whitespace());
    is_hash_comment.then_some(i)
}

fn consume_quoted(chars: &[char], start: usize, quote: char) -> (String, usize) {
    let mut token = String::new();
    token.push(quote);
    let mut i = start + 1;
    let mut escaped = false;

    while i < chars.len() {
        let ch = chars[i];
        token.push(ch);
        i += 1;

        if escaped {
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        if ch == quote {
            break;
        }
    }

    (token, i)
}

fn consume_number(chars: &[char], start: usize) -> (String, usize) {
    let mut token = String::new();
    let mut i = start;

    while i < chars.len() {
        let ch = chars[i];
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.') {
            token.push(ch);
            i += 1;
        } else {
            break;
        }
    }

    (token, i)
}

fn consume_ident(chars: &[char], start: usize) -> (String, usize) {
    let mut token = String::new();
    let mut i = start;

    while i < chars.len() && is_ident_continue(chars[i]) {
        token.push(chars[i]);
        i += 1;
    }

    (token, i)
}

fn classify_ident(token: &str, theme: Theme) -> Option<Style> {
    if KEYWORDS.contains(&token) {
        Some(keyword_style(theme))
    } else if token
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
    {
        Some(type_style(theme))
    } else {
        None
    }
}

fn flush_plain<'a>(spans: &mut Vec<Span<'a>>, plain: &mut String, theme: Theme) {
    if !plain.is_empty() {
        spans.push(Span::styled(std::mem::take(plain), plain_style(theme)));
    }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn plain_style(theme: Theme) -> Style {
    Style::default().fg(theme.text)
}

fn keyword_style(theme: Theme) -> Style {
    Style::default()
        .fg(theme.border_focus)
        .add_modifier(Modifier::BOLD)
}

fn type_style(theme: Theme) -> Style {
    Style::default().fg(theme.list_highlight_fg)
}

fn string_style(theme: Theme) -> Style {
    Style::default().fg(theme.accent_info)
}

fn number_style(theme: Theme) -> Style {
    Style::default().fg(theme.accent_warn)
}

fn comment_style(theme: Theme) -> Style {
    Style::default()
        .fg(theme.muted)
        .add_modifier(Modifier::ITALIC)
}
