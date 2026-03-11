use muffintui::{syntax::highlight_line, theme::THEMES};
use ratatui::style::{Color, Modifier};

#[test]
fn highlights_keywords_strings_numbers_and_comments() {
    let line = highlight_line(r#"let total = 42; // "note""#, THEMES[0]);

    assert_eq!(line.spans[0].content.as_ref(), "let");
    assert_eq!(line.spans[0].style.fg, Some(THEMES[0].border_focus));
    assert!(line.spans[0].style.add_modifier.contains(Modifier::BOLD));

    let number = line
        .spans
        .iter()
        .find(|span| span.content.as_ref() == "42")
        .unwrap();
    assert_eq!(number.style.fg, Some(THEMES[0].accent_warn));

    let comment = line
        .spans
        .iter()
        .find(|span| span.content.as_ref().starts_with("//"))
        .unwrap();
    assert_eq!(comment.style.fg, Some(THEMES[0].muted));
    assert!(comment.style.add_modifier.contains(Modifier::ITALIC));
}

#[test]
fn highlights_uppercase_identifiers_as_types() {
    let line = highlight_line("Option<String>", THEMES[1]);

    let option = &line.spans[0];
    assert_eq!(option.content.as_ref(), "Option");
    assert_eq!(option.style.fg, Some(THEMES[1].list_highlight_fg));
}

#[test]
fn leaves_plain_text_in_base_color() {
    let line = highlight_line("value = other", THEMES[2]);
    let plain = line
        .spans
        .iter()
        .find(|span| span.content.as_ref().contains("value"))
        .unwrap();
    assert_eq!(plain.style.fg, Some(THEMES[2].text));
    assert_ne!(plain.style.fg, Some(Color::Reset));
}
