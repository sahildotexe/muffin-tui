use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use muffintui::terminal::{
    bytes_to_lines, handle_scrollback_key, is_terminal_clear_command, push_capped_line,
    run_shell_command, update_input_buffer,
};

#[test]
fn detects_clear_commands_for_builtin_terminal() {
    assert!(is_terminal_clear_command("clear"));
    assert!(is_terminal_clear_command("clear;"));
    assert!(is_terminal_clear_command("cls"));
    assert!(!is_terminal_clear_command("clear now"));
    assert!(!is_terminal_clear_command("printf '\\033[2J'"));
}

#[test]
fn strips_ansi_escape_sequences_from_shell_output() {
    let lines = bytes_to_lines(b"\x1b[H\x1b[2Jclean\r\n\x1b]0;title\x07next\r\n");
    assert_eq!(lines, vec!["clean", "next"]);
}

#[test]
fn update_input_buffer_collects_and_submits_text() {
    let mut buffer = String::new();
    assert_eq!(
        update_input_buffer(
            &mut buffer,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)
        ),
        None
    );
    assert_eq!(buffer, "a");

    let submitted = update_input_buffer(
        &mut buffer,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    );
    assert_eq!(submitted.as_deref(), Some("a"));
    assert!(buffer.is_empty());
}

#[test]
fn scrollback_keys_adjust_position() {
    let mut scroll = 0;
    assert!(handle_scrollback_key(
        KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
        &mut scroll
    ));
    assert_eq!(scroll, 8);

    assert!(handle_scrollback_key(
        KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
        &mut scroll
    ));
    assert_eq!(scroll, 0);
}

#[test]
fn push_capped_line_keeps_recent_history() {
    let mut lines = Vec::new();
    for i in 0..505 {
        push_capped_line(&mut lines, i.to_string());
    }

    assert_eq!(lines.len(), 500);
    assert_eq!(lines.first().map(String::as_str), Some("5"));
    assert_eq!(lines.last().map(String::as_str), Some("504"));
}

#[test]
fn run_shell_command_captures_output() {
    let lines = run_shell_command("printf 'hello\\n'");
    assert_eq!(lines, vec!["hello"]);
}
