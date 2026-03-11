use muffintui::{
    codex::{ansi_256_to_ratatui, color_from_vt100, io_other},
    theme::THEMES,
};
use ratatui::style::Color;
use std::io::ErrorKind;

#[test]
fn translates_default_vt100_colors_from_theme() {
    let theme = THEMES[0];
    assert_eq!(color_from_vt100(vt100::Color::Default, false, theme), theme.text);
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
