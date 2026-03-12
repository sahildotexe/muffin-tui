use muffintui::{
    codex::{SessionMode, ansi_256_to_ratatui, color_from_vt100, io_other},
    theme::THEMES,
};
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

#[test]
fn session_modes_expose_expected_titles_and_statuses() {
    assert_eq!(SessionMode::Shell.pane_title(), "Shell");
    assert_eq!(SessionMode::Codex.pane_title(), "Codex");
    assert_eq!(SessionMode::Claude.pane_title(), "Claude");

    assert_eq!(
        SessionMode::Shell.success_status(),
        "Shell session connected"
    );
    assert_eq!(
        SessionMode::Codex.success_status(),
        "Codex session connected"
    );
    assert_eq!(
        SessionMode::Claude.success_status(),
        "Claude session connected"
    );
}

#[test]
fn shell_mode_uses_shell_env_or_fallback() {
    let shell = SessionMode::Shell.command();
    assert!(!shell.is_empty());

    assert_eq!(SessionMode::Codex.command(), "codex");
    assert_eq!(SessionMode::Claude.command(), "claude");
}
