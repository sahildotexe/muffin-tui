use std::{
    collections::HashSet,
    fs, io,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Focus {
    FileTree,
    Editor,
    Terminal,
    Codex,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Self::FileTree => Self::Editor,
            Self::Editor => Self::Terminal,
            Self::Terminal => Self::Codex,
            Self::Codex => Self::FileTree,
        }
    }
}

struct FileEntry {
    path: PathBuf,
    display: String,
    is_dir: bool,
    depth: usize,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum EditorMode {
    Normal,
    Diff,
}

impl EditorMode {
    fn toggle(self) -> Self {
        match self {
            Self::Normal => Self::Diff,
            Self::Diff => Self::Normal,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Diff => "Diff",
        }
    }
}

struct App {
    running: bool,
    focus: Focus,
    theme_index: usize,
    root_dir: PathBuf,
    expanded_dirs: HashSet<PathBuf>,
    files: Vec<FileEntry>,
    file_state: ListState,
    editor_path: Option<PathBuf>,
    editor_mode: EditorMode,
    editor_title: String,
    editor_lines: Vec<String>,
    editor_scroll: usize,
    terminal_output: Vec<String>,
    terminal_input: String,
    terminal_scroll: usize,
    codex: Option<CodexSession>,
    codex_status: String,
}

impl App {
    fn new() -> io::Result<Self> {
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
            expanded_dirs,
            files,
            file_state,
            editor_path: None,
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
        })
    }

    fn on_tick(&mut self) {}

    fn on_key(&mut self, key: KeyEvent) {
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
                if handle_terminal_scrollback_key(key, &mut self.terminal_scroll) {
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
        let i = self.file_state.selected().unwrap_or(0);
        let next = if i == 0 { self.files.len() - 1 } else { i - 1 };
        self.file_state.select(Some(next));
    }

    fn next_file(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let i = self.file_state.selected().unwrap_or(0);
        let next = if i >= self.files.len() - 1 { 0 } else { i + 1 };
        self.file_state.select(Some(next));
    }

    fn open_selected_file(&mut self) {
        let Some(i) = self.file_state.selected() else {
            return;
        };
        let Some(entry) = self.files.get(i) else {
            return;
        };
        let path = entry.path.clone();
        let display = entry.display.clone();
        let is_dir = entry.is_dir;

        if is_dir {
            self.toggle_directory(&path);
            return;
        }

        self.editor_path = Some(path.clone());
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

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();
    let mut app = App::new()?;

    while app.running {
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                app.on_key(key);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }
    }

    Ok(())
}

#[derive(Copy, Clone)]
struct Theme {
    name: &'static str,
    app_bg: Color,
    pane_bg: Color,
    border: Color,
    border_focus: Color,
    title: Color,
    title_focus_fg: Color,
    title_focus_bg: Color,
    text: Color,
    muted: Color,
    list_highlight_bg: Color,
    list_highlight_fg: Color,
    codex_cursor: Color,
    accent_warn: Color,
    accent_info: Color,
    accent_scroll: Color,
}

const THEMES: [Theme; 3] = [
    Theme {
        name: "Teal Night",
        app_bg: Color::Rgb(8, 11, 15),
        pane_bg: Color::Rgb(11, 14, 18),
        border: Color::Rgb(92, 104, 118),
        border_focus: Color::Rgb(95, 211, 188),
        title: Color::Rgb(180, 188, 196),
        title_focus_fg: Color::Black,
        title_focus_bg: Color::Rgb(95, 211, 188),
        text: Color::Rgb(232, 238, 242),
        muted: Color::Rgb(130, 145, 160),
        list_highlight_bg: Color::Rgb(39, 47, 58),
        list_highlight_fg: Color::Rgb(255, 224, 102),
        codex_cursor: Color::Rgb(95, 211, 188),
        accent_warn: Color::Rgb(241, 196, 15),
        accent_info: Color::Rgb(80, 227, 194),
        accent_scroll: Color::Rgb(137, 180, 250),
    },
    Theme {
        name: "Amber Graphite",
        app_bg: Color::Rgb(18, 15, 12),
        pane_bg: Color::Rgb(24, 21, 17),
        border: Color::Rgb(122, 104, 90),
        border_focus: Color::Rgb(255, 176, 90),
        title: Color::Rgb(205, 190, 174),
        title_focus_fg: Color::Black,
        title_focus_bg: Color::Rgb(255, 176, 90),
        text: Color::Rgb(244, 235, 225),
        muted: Color::Rgb(169, 152, 136),
        list_highlight_bg: Color::Rgb(58, 44, 31),
        list_highlight_fg: Color::Rgb(255, 220, 160),
        codex_cursor: Color::Rgb(255, 176, 90),
        accent_warn: Color::Rgb(255, 213, 79),
        accent_info: Color::Rgb(255, 142, 83),
        accent_scroll: Color::Rgb(186, 230, 126),
    },
    Theme {
        name: "Ice Blue",
        app_bg: Color::Rgb(232, 238, 245),
        pane_bg: Color::Rgb(244, 248, 252),
        border: Color::Rgb(132, 151, 170),
        border_focus: Color::Rgb(50, 127, 191),
        title: Color::Rgb(82, 100, 119),
        title_focus_fg: Color::White,
        title_focus_bg: Color::Rgb(50, 127, 191),
        text: Color::Rgb(20, 35, 48),
        muted: Color::Rgb(96, 116, 136),
        list_highlight_bg: Color::Rgb(205, 223, 240),
        list_highlight_fg: Color::Rgb(16, 57, 95),
        codex_cursor: Color::Rgb(50, 127, 191),
        accent_warn: Color::Rgb(232, 180, 44),
        accent_info: Color::Rgb(95, 168, 211),
        accent_scroll: Color::Rgb(117, 197, 143),
    },
];

fn pane_block(title: &str, focused: bool, theme: Theme) -> Block<'static> {
    let style = if focused {
        Style::default()
            .fg(theme.border_focus)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.border)
    };

    Block::default()
        .borders(Borders::ALL)
        .border_style(style)
        .style(Style::default().bg(theme.pane_bg).fg(theme.text))
        .title(Span::styled(
            title.to_string(),
            if focused {
                Style::default()
                    .fg(theme.title_focus_fg)
                    .bg(theme.title_focus_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.title).bg(theme.pane_bg)
            },
        ))
}

fn ui(frame: &mut Frame, app: &mut App) {
    let root = frame.area();
    let theme = THEMES[app.theme_index];

    frame.render_widget(
        Block::default().style(Style::default().bg(theme.app_bg)),
        root,
    );

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(22),
            Constraint::Percentage(53),
            Constraint::Percentage(25),
        ])
        .split(root);

    let middle = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(9)])
        .split(columns[1]);

    let file_items: Vec<ListItem> = app
        .files
        .iter()
        .map(|f| {
            let indent = "  ".repeat(f.depth);
            ListItem::new(Line::from(format!("{indent}{}", f.display)))
        })
        .collect();

    let file_list = List::new(file_items)
        .block(pane_block("Files", app.focus == Focus::FileTree, theme))
        .style(Style::default().bg(theme.pane_bg).fg(theme.text))
        .highlight_style(
            Style::default()
                .bg(theme.list_highlight_bg)
                .fg(theme.list_highlight_fg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(file_list, columns[0], &mut app.file_state);

    let editor_height = middle[0].height.saturating_sub(2) as usize;
    let max_editor_scroll = app.editor_lines.len().saturating_sub(editor_height.max(1));
    app.editor_scroll = app.editor_scroll.min(max_editor_scroll);

    let editor_text = app
        .editor_lines
        .iter()
        .skip(app.editor_scroll)
        .take(editor_height.max(1))
        .map(|line| editor_line(line, app.editor_mode, theme))
        .collect::<Vec<_>>();

    let editor = Paragraph::new(editor_text)
        .style(Style::default().bg(theme.pane_bg).fg(theme.text))
        .wrap(Wrap { trim: false })
        .block(pane_block(
            &app.editor_title,
            app.focus == Focus::Editor,
            theme,
        ));

    frame.render_widget(editor, middle[0]);

    let terminal_height = middle[1].height.saturating_sub(2) as usize;
    let mut terminal_lines = app
        .terminal_output
        .iter()
        .map(|line| Line::from(line.as_str()))
        .collect::<Vec<_>>();
    terminal_lines.push(Line::from(vec![
        Span::styled("$ ", Style::default().fg(theme.border_focus)),
        Span::styled(app.terminal_input.as_str(), Style::default().fg(theme.text)),
    ]));
    let max_terminal_scroll = terminal_lines.len().saturating_sub(terminal_height.max(1));
    app.terminal_scroll = app.terminal_scroll.min(max_terminal_scroll);
    let end = terminal_lines.len().saturating_sub(app.terminal_scroll);
    let start = end.saturating_sub(terminal_height.max(1));
    let terminal_lines = terminal_lines
        .into_iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect::<Vec<_>>();

    let terminal_pane = Paragraph::new(terminal_lines)
        .style(Style::default().bg(theme.pane_bg).fg(theme.text))
        .wrap(Wrap { trim: false })
        .block(pane_block("Terminal", app.focus == Focus::Terminal, theme));

    frame.render_widget(terminal_pane, middle[1]);

    let codex_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(columns[2]);

    let codex_header_style = if app.focus == Focus::Codex {
        Style::default()
            .fg(theme.title_focus_fg)
            .bg(theme.title_focus_bg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.title).bg(theme.pane_bg)
    };

    let codex_header = Paragraph::new(Line::from(vec![
        Span::styled(" Codex ", codex_header_style),
        Span::styled(
            format!("live session  [{0}]  Shift+Tab theme", theme.name),
            Style::default().fg(theme.muted).bg(theme.pane_bg),
        ),
    ]))
    .style(Style::default().bg(theme.pane_bg));
    frame.render_widget(codex_header, codex_chunks[0]);

    frame.render_widget(
        Block::default().style(Style::default().bg(theme.pane_bg)),
        codex_chunks[1],
    );

    let codex_output_lines = if let Some(codex) = app.codex.as_mut() {
        let width = codex_chunks[1].width.max(1);
        let height = codex_chunks[1].height.max(1);
        let _ = codex.resize(width, height);
        codex.snapshot_lines(height, width, theme)
    } else {
        vec![Line::styled(
            app.codex_status.as_str(),
            Style::default().fg(theme.muted).bg(theme.pane_bg),
        )]
    };

    let codex = Paragraph::new(codex_output_lines).style(Style::default().bg(theme.pane_bg));

    frame.render_widget(codex, codex_chunks[1]);

    let codex_footer = Paragraph::new(Line::from(vec![
        Span::styled(
            " Ctrl+C ",
            Style::default()
                .fg(theme.title_focus_fg)
                .bg(theme.accent_warn)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "interrupt",
            Style::default().fg(theme.muted).bg(theme.pane_bg),
        ),
        Span::raw("  "),
        Span::styled(
            " Tab ",
            Style::default()
                .fg(theme.title_focus_fg)
                .bg(theme.accent_info),
        ),
        Span::styled(
            "leave pane",
            Style::default().fg(theme.muted).bg(theme.pane_bg),
        ),
        Span::raw("  "),
        Span::styled(
            " PgUp/PgDn ",
            Style::default()
                .fg(theme.title_focus_fg)
                .bg(theme.accent_scroll),
        ),
        Span::styled("scroll", Style::default().fg(theme.muted).bg(theme.pane_bg)),
    ]))
    .style(Style::default().bg(theme.pane_bg));

    frame.render_widget(codex_footer, codex_chunks[2]);
}

fn update_input_buffer(buffer: &mut String, key: KeyEvent) -> Option<String> {
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

fn run_shell_command(cmd: &str) -> Vec<String> {
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

fn bytes_to_lines(bytes: &[u8]) -> Vec<String> {
    let text = strip_ansi_escape_sequences(&String::from_utf8_lossy(bytes));
    let mut lines: Vec<String> = text.lines().map(ToOwned::to_owned).collect();
    if lines.is_empty() && !text.trim().is_empty() {
        lines.push(text.trim().to_string());
    }
    lines
}

fn is_terminal_clear_command(cmd: &str) -> bool {
    matches!(cmd, "clear" | "clear;" | "cls" | "cls;")
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
                while let Some(next) = chars.next() {
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

fn push_capped_line(lines: &mut Vec<String>, line: String) {
    const MAX_LINES: usize = 500;
    lines.push(line);
    if lines.len() > MAX_LINES {
        let overflow = lines.len() - MAX_LINES;
        lines.drain(0..overflow);
    }
}

struct CodexSession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn io::Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    parser: Arc<Mutex<vt100::Parser>>,
}

impl CodexSession {
    fn start(cwd: &Path, cols: u16, rows: u16) -> io::Result<Self> {
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

    fn send_ctrl_c(&mut self) -> io::Result<()> {
        self.writer.write_all(&[0x03])?;
        self.writer.flush()
    }

    fn send_key(&mut self, key: KeyEvent) -> io::Result<()> {
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

    fn resize(&mut self, cols: u16, rows: u16) -> io::Result<()> {
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

    fn snapshot_lines(&self, rows: u16, cols: u16, theme: Theme) -> Vec<Line<'static>> {
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

impl Drop for CodexSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn handle_terminal_scrollback_key(key: KeyEvent, terminal_scroll: &mut usize) -> bool {
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

#[cfg(test)]
mod tests {
    use super::{bytes_to_lines, is_terminal_clear_command};

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
}

fn editor_line<'a>(line: &'a str, mode: EditorMode, theme: Theme) -> Line<'a> {
    if mode == EditorMode::Diff {
        let style = if line.starts_with('+') && !line.starts_with("+++") {
            Style::default().fg(Color::Rgb(108, 214, 141))
        } else if line.starts_with('-') && !line.starts_with("---") {
            Style::default().fg(Color::Rgb(255, 120, 117))
        } else if line.starts_with("@@") {
            Style::default()
                .fg(theme.border_focus)
                .add_modifier(Modifier::BOLD)
        } else if line.starts_with("diff --git")
            || line.starts_with("index ")
            || line.starts_with("---")
            || line.starts_with("+++")
        {
            Style::default().fg(theme.muted)
        } else {
            Style::default().fg(theme.text)
        };
        Line::styled(line.to_string(), style)
    } else {
        Line::styled(line.to_string(), Style::default().fg(theme.text))
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

    let mut lines = bytes_to_lines(&output.stdout);
    if lines.is_empty() {
        lines.push(format!(
            "No changes in {} compared with HEAD~1.",
            relative_path.display()
        ));
    }
    Ok(lines)
}

fn io_other<E: ToString>(err: E) -> io::Error {
    io::Error::other(err.to_string())
}

fn collect_visible_file_entries(
    root: &Path,
    expanded_dirs: &HashSet<PathBuf>,
) -> io::Result<Vec<FileEntry>> {
    let mut out = Vec::new();
    collect_visible_file_entries_recursive(root, root, expanded_dirs, 0, &mut out)?;
    Ok(out)
}

fn collect_visible_file_entries_recursive(
    root: &Path,
    dir: &Path,
    expanded_dirs: &HashSet<PathBuf>,
    depth: usize,
    out: &mut Vec<FileEntry>,
) -> io::Result<()> {
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, io::Error>>()?;
    entries.sort_by(|a, b| {
        let a_is_dir = a.file_type().map(|f| f.is_dir()).unwrap_or(false);
        let b_is_dir = b.file_type().map(|f| f.is_dir()).unwrap_or(false);
        b_is_dir
            .cmp(&a_is_dir)
            .then_with(|| a.file_name().cmp(&b.file_name()))
    });

    for entry in entries {
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_name == ".git" || file_name == "target" {
            continue;
        }

        let is_dir = entry.file_type()?.is_dir();
        let icon = if is_dir {
            if expanded_dirs.contains(&path) {
                "▾"
            } else {
                "▸"
            }
        } else {
            " "
        };
        let display = if is_dir {
            format!("{icon} {file_name}/")
        } else {
            format!("{icon} {file_name}")
        };

        out.push(FileEntry {
            path: path.clone(),
            display,
            is_dir,
            depth,
        });

        if is_dir && expanded_dirs.contains(&path) {
            collect_visible_file_entries_recursive(root, &path, expanded_dirs, depth + 1, out)?;
        }
    }

    if depth == 0 && out.is_empty() {
        out.push(FileEntry {
            path: root.to_path_buf(),
            display: "(empty)".to_string(),
            is_dir: false,
            depth: 0,
        });
    }

    Ok(())
}

fn collapse_directory(path: &Path, expanded_dirs: &mut HashSet<PathBuf>) {
    expanded_dirs.retain(|candidate| candidate != path && !candidate.starts_with(path));
}
