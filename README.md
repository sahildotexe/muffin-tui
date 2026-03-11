# muffintui

`muffintui` is a terminal workspace for code navigation and command execution with four panes:

- Files
- Editor
- Terminal
- Codex

It is designed to be launched inside any project directory. The current working directory becomes:

- the root of the Files pane
- the working directory for shell commands
- the working directory for the embedded Codex session

## Requirements

- Rust and Cargo
- `codex` installed and available on `PATH`

## Install

Install from the local repository:

```bash
cargo install --path .
```

After publishing to `crates.io`, users will be able to install it with:

```bash
cargo install muffintui
```

## Run

Run in the current directory:

```bash
muffintui
```

Run against another project:

```bash
cd /path/to/project
muffintui
```

For local development without reinstalling:

```bash
cargo run
```

## Update After Changes

Reinstall the current local source:

```bash
cargo install --path .
```

## Keybindings

### Global

- `Tab`: move focus to the next pane
- `Shift+Tab`: cycle the UI theme
- `Esc`: quit
- `Ctrl+C`: quit when not focused on Codex

### Files Pane

- `Up` or `k`: move selection up
- `Down` or `j`: move selection down
- `Enter` on a folder: expand or collapse that folder in place
- `Enter` on a file: open the file in the editor

### Editor Pane

- `PageUp`: scroll up
- `PageDown`: scroll down
- `Ctrl+D`: toggle between normal view and diff view

Editor modes:

- `Normal`: show the current file contents
- `Diff`: show `git diff HEAD~1 -- <file>` for the selected file

If the repository does not have a previous commit, diff mode shows a fallback message instead.

### Terminal Pane

- Type any shell command directly
- `Enter`: run the command
- `Backspace`: delete one character from the prompt
- `PageUp`: scroll terminal history up
- `PageDown`: scroll terminal history down
- `Home`: jump to the oldest visible terminal history
- `End`: jump back to the live prompt/output

When a command runs, the terminal automatically snaps back to the latest output.

### Codex Pane

- Regular typing: send keystrokes directly to the embedded `codex` session
- `Enter`: submit input to Codex
- `Ctrl+C`: send interrupt to Codex instead of quitting the TUI

Note:

- Codex pane scrolling is currently handled by the embedded Codex application itself, not by `muffintui`

## Publishing

Publish with:

```bash
cargo login
cargo package
cargo publish
```
