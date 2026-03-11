use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::widgets::ListState;

use crate::{
    codex::CodexSession,
    file_tree::{FileEntry, collect_visible_file_entries, collapse_directory},
    terminal::{
        handle_scrollback_key, is_terminal_clear_command, push_capped_line, run_shell_command,
        update_input_buffer,
    },
    theme::THEMES,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Focus {
    FileTree,
    Editor,
    Terminal,
    Codex,
}

impl Focus {
    pub fn next(self) -> Self {
        match self {
            Self::FileTree => Self::Editor,
            Self::Editor => Self::Terminal,
            Self::Terminal => Self::Codex,
            Self::Codex => Self::FileTree,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EditorMode {
    Normal,
    Diff,
}

impl EditorMode {
    pub fn toggle(self) -> Self {
        match self {
            Self::Normal => Self::Diff,
            Self::Diff => Self::Normal,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Diff => "Diff",
        }
    }
}

pub struct App {
    pub running: bool,
    pub focus: Focus,
    pub theme_index: usize,
    pub root_dir: PathBuf,
    pub files: Vec<FileEntry>,
    pub file_state: ListState,
    pub editor_mode: EditorMode,
    pub editor_title: String,
    pub editor_lines: Vec<String>,
    pub editor_scroll: usize,
    pub terminal_output: Vec<String>,
    pub terminal_input: String,
    pub terminal_scroll: usize,
    pub codex: Option<CodexSession>,
    pub codex_status: String,
    expanded_dirs: HashSet<PathBuf>,
    editor_path: Option<PathBuf>,
}

impl App {
    pub fn new() -> io::Result<Self> {
        let cwd = std::env::current_dir()?;
        let expanded_dirs = HashSet::new();
        let files = collect_visible_file_entries(&cwd, &expanded_dirs)?;
        let mut file_state = ListState::default();
        file_state.select((!files.is_empty()).then_some(0));

        let (codex, codex_status) = match CodexSession::start(&cwd, 80, 24) {
            Ok(session) => (Some(session), "Codex session connected".to_string()),
            Err(err) => (None, format!("Failed to start codex session: {}", err)),
        };

        Ok(Self {
            running: true,
            focus: Focus::Editor,
            theme_index: 0,
            root_dir: cwd,
            files,
            file_state,
            editor_mode: EditorMode::Normal,
            editor_title: "Editor".to_string(),
            editor_lines: vec![
                "// Editor pane".to_string(),
                "// Select a file in the left pane and press Enter to open it.".to_string(),
            ],
            editor_scroll: 0,
            terminal_output: vec![
                "$ cargo run".to_string(),
                "Launching muffintui...".to_string(),
            ],
            terminal_input: String::new(),
            terminal_scroll: 0,
            codex,
            codex_status,
            expanded_dirs,
            editor_path: None,
        })
    }

    pub fn on_tick(&mut self) {}

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            if self.focus == Focus::Codex {
                if let Some(codex) = self.codex.as_mut() {
                    if let Err(err) = codex.send_ctrl_c() {
                        self.codex_status = format!("Failed to send Ctrl+C to codex: {}", err);
                    }
                }
            } else {
                self.running = false;
            }
            return;
        }

        match key.code {
            KeyCode::Esc => self.running = false,
            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::BackTab => self.theme_index = (self.theme_index + 1) % THEMES.len(),
            _ => self.handle_focused_input(key),
        }
    }

    fn handle_focused_input(&mut self, key: KeyEvent) {
        match self.focus {
            Focus::FileTree => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.prev_file(),
                KeyCode::Down | KeyCode::Char('j') => self.next_file(),
                KeyCode::Enter => self.open_selected_file(),
                _ => {}
            },
            Focus::Terminal => {
                if handle_scrollback_key(key, &mut self.terminal_scroll) {
                    return;
                }
                if let Some(cmd) = update_input_buffer(&mut self.terminal_input, key) {
                    self.run_terminal_command(&cmd);
                }
            }
            Focus::Codex => {
                if let Some(codex) = self.codex.as_mut() {
                    if let Err(err) = codex.send_key(key) {
                        self.codex_status = format!("Codex input error: {}", err);
                    }
                } else if key.code == KeyCode::Enter {
                    match CodexSession::start(&self.root_dir, 80, 24) {
                        Ok(session) => {
                            self.codex = Some(session);
                            self.codex_status = "Codex session connected".to_string();
                        }
                        Err(err) => {
                            self.codex_status = format!("Failed to start codex session: {}", err);
                        }
                    }
                }
            }
            Focus::Editor => match key.code {
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.toggle_editor_mode();
                }
                KeyCode::PageUp => self.editor_scroll = self.editor_scroll.saturating_sub(10),
                KeyCode::PageDown => {
                    self.editor_scroll = self.editor_scroll.saturating_add(10);
                }
                _ => {}
            },
        }
    }

    fn prev_file(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let index = self.file_state.selected().unwrap_or(0);
        let next = if index == 0 {
            self.files.len() - 1
        } else {
            index - 1
        };
        self.file_state.select(Some(next));
    }

    fn next_file(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let index = self.file_state.selected().unwrap_or(0);
        let next = if index >= self.files.len() - 1 {
            0
        } else {
            index + 1
        };
        self.file_state.select(Some(next));
    }

    fn open_selected_file(&mut self) {
        let Some(index) = self.file_state.selected() else {
            return;
        };
        let Some(entry) = self.files.get(index) else {
            return;
        };
        let path = entry.path.clone();
        let display = entry.display.clone();

        if entry.is_dir {
            self.toggle_directory(&path);
            return;
        }

        self.editor_path = Some(path);
        if let Err(err) = self.reload_editor_contents() {
            push_capped_line(
                &mut self.terminal_output,
                format!("Failed to open {}: {}", display, err),
            );
            return;
        }

        self.focus = Focus::Editor;
    }

    fn toggle_directory(&mut self, path: &Path) {
        if self.expanded_dirs.contains(path) {
            collapse_directory(path, &mut self.expanded_dirs);
        } else {
            self.expanded_dirs.insert(path.to_path_buf());
        }

        match collect_visible_file_entries(&self.root_dir, &self.expanded_dirs) {
            Ok(files) => {
                self.files = files;
                let selected = self
                    .files
                    .iter()
                    .position(|entry| entry.path == path)
                    .or_else(|| (!self.files.is_empty()).then_some(0));
                self.file_state.select(selected);
            }
            Err(err) => {
                push_capped_line(
                    &mut self.terminal_output,
                    format!("Failed to read {}: {}", path.display(), err),
                );
            }
        }
    }

    fn run_terminal_command(&mut self, cmd: &str) {
        let trimmed = cmd.trim();
        if trimmed.is_empty() {
            return;
        }

        if is_terminal_clear_command(trimmed) {
            self.terminal_output.clear();
            self.terminal_scroll = 0;
            return;
        }

        push_capped_line(&mut self.terminal_output, format!("$ {}", trimmed));
        let output = run_shell_command(trimmed);
        if output.is_empty() {
            push_capped_line(&mut self.terminal_output, "(no output)".to_string());
        } else {
            for line in output {
                push_capped_line(&mut self.terminal_output, line);
            }
        }
        self.terminal_scroll = 0;
    }

    fn toggle_editor_mode(&mut self) {
        self.editor_mode = self.editor_mode.toggle();
        if let Err(err) = self.reload_editor_contents() {
            push_capped_line(
                &mut self.terminal_output,
                format!("Failed to load {} mode: {}", self.editor_mode.label(), err),
            );
        }
    }

    fn reload_editor_contents(&mut self) -> io::Result<()> {
        let Some(path) = self.editor_path.as_ref() else {
            self.editor_title = format!("Editor [{}]", self.editor_mode.label());
            self.editor_lines = vec!["Open a file to view it.".to_string()];
            self.editor_scroll = 0;
            return Ok(());
        };

        let relative = path.strip_prefix(&self.root_dir).unwrap_or(path);
        self.editor_lines = match self.editor_mode {
            EditorMode::Normal => read_file_lines(path)?,
            EditorMode::Diff => git_diff_for_file(&self.root_dir, relative)?,
        };
        self.editor_title = format!(
            "Editor - {} [{}] Ctrl+D toggle",
            relative.display(),
            self.editor_mode.label()
        );
        self.editor_scroll = 0;
        Ok(())
    }
}

impl App {
    #[doc(hidden)]
    pub fn test_fixture() -> Self {
        let mut file_state = ListState::default();
        file_state.select(Some(0));
        Self {
            running: true,
            focus: Focus::Editor,
            theme_index: 0,
            root_dir: std::env::temp_dir(),
            files: vec![FileEntry {
                path: PathBuf::from("example.txt"),
                display: "  example.txt".to_string(),
                is_dir: false,
                depth: 0,
            }],
            file_state,
            editor_mode: EditorMode::Normal,
            editor_title: "Editor".to_string(),
            editor_lines: vec!["hello".to_string()],
            editor_scroll: 0,
            terminal_output: vec!["existing".to_string()],
            terminal_input: String::new(),
            terminal_scroll: 0,
            codex: None,
            codex_status: "Failed to start codex session".to_string(),
            expanded_dirs: HashSet::new(),
            editor_path: None,
        }
    }
}

fn read_file_lines(path: &Path) -> io::Result<Vec<String>> {
    let content = fs::read_to_string(path)?;
    Ok(if content.is_empty() {
        vec!["".to_string()]
    } else {
        content.lines().map(ToOwned::to_owned).collect()
    })
}

fn git_diff_for_file(root_dir: &Path, relative_path: &Path) -> io::Result<Vec<String>> {
    let previous_commit = Command::new("git")
        .arg("rev-parse")
        .arg("--verify")
        .arg("HEAD~1")
        .current_dir(root_dir)
        .output()?;

    if !previous_commit.status.success() {
        return Ok(vec![
            "No previous commit available for diff mode.".to_string(),
            format!("File: {}", relative_path.display()),
        ]);
    }

    let output = Command::new("git")
        .arg("diff")
        .arg("HEAD~1")
        .arg("--")
        .arg(relative_path)
        .current_dir(root_dir)
        .output()?;

    if !output.status.success() {
        return Ok(vec![
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ]);
    }

    let mut lines = crate::terminal::bytes_to_lines(&output.stdout);
    if lines.is_empty() {
        lines.push(format!(
            "No changes in {} compared with HEAD~1.",
            relative_path.display()
        ));
    }
    Ok(lines)
}
