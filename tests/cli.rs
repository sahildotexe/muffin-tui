use muffintui::{cli::parse_session_mode, codex::SessionMode};

#[test]
fn defaults_to_shell_when_no_flag_is_passed() {
    let mode = parse_session_mode(std::iter::empty::<String>()).unwrap();
    assert_eq!(mode, SessionMode::Shell);
}

#[test]
fn parses_codex_flag() {
    let mode = parse_session_mode(["--codex".to_string()].into_iter()).unwrap();
    assert_eq!(mode, SessionMode::Codex);
}

#[test]
fn parses_claude_flag() {
    let mode = parse_session_mode(["--claude".to_string()].into_iter()).unwrap();
    assert_eq!(mode, SessionMode::Claude);
}

#[test]
fn rejects_multiple_session_flags() {
    let err = parse_session_mode(["--codex".to_string(), "--claude".to_string()].into_iter())
        .unwrap_err();
    assert!(err.to_string().contains("choose only one"));
}

#[test]
fn rejects_unknown_arguments() {
    let err = parse_session_mode(["--wat".to_string()].into_iter()).unwrap_err();
    assert!(err.to_string().contains("unknown argument"));
}
