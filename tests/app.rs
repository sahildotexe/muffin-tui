use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use muffintui::{
    app::{App, EditorMode, Focus},
    file_tree::FileEntry,
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_test_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("muffin-app-{name}-{nanos}"))
}

#[test]
fn focus_cycles_across_all_panes() {
    assert_eq!(Focus::FileTree.next(), Focus::Editor);
    assert_eq!(Focus::Editor.next(), Focus::Terminal);
    assert_eq!(Focus::Terminal.next(), Focus::Codex);
    assert_eq!(Focus::Codex.next(), Focus::FileTree);
}

#[test]
fn editor_mode_toggles_and_labels() {
    assert_eq!(EditorMode::Normal.toggle(), EditorMode::Diff);
    assert_eq!(EditorMode::Diff.toggle(), EditorMode::Normal);
    assert_eq!(EditorMode::Normal.label(), "Normal");
    assert_eq!(EditorMode::Diff.label(), "Diff");
}

#[test]
fn global_keys_update_focus_and_theme() {
    let mut app = App::test_fixture();
    app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.focus, Focus::Terminal);

    app.on_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert_eq!(app.theme_index, 1);
}

#[test]
fn ctrl_c_outside_codex_stops_app() {
    let mut app = App::test_fixture();
    app.focus = Focus::Terminal;
    app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(!app.running);
}

#[test]
fn clear_command_only_clears_terminal_pane() {
    let mut app = App::test_fixture();
    app.focus = Focus::Terminal;
    app.editor_lines = vec!["keep me".to_string()];
    app.terminal_input = "clear".to_string();

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(app.terminal_output.is_empty());
    assert_eq!(app.editor_lines, vec!["keep me"]);
}

#[test]
fn opens_selected_file_into_editor() {
    let root = temp_test_dir("open-file");
    fs::create_dir_all(&root).unwrap();
    let file_path = root.join("notes.txt");
    fs::write(&file_path, "first\nsecond\n").unwrap();

    let mut app = App::test_fixture();
    app.root_dir = root.clone();
    app.files = vec![FileEntry {
        path: file_path,
        display: "  notes.txt".to_string(),
        is_dir: false,
        depth: 0,
    }];
    app.focus = Focus::FileTree;

    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(app.focus, Focus::Editor);
    assert_eq!(app.editor_title, "File Viewer - notes.txt [Normal] Ctrl+D toggle");
    assert_eq!(app.editor_lines, vec!["first".to_string(), "second".to_string()]);

    fs::remove_dir_all(root).unwrap();
}
