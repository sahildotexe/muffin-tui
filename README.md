# muffintui

`muffintui` is the crates.io package for a Rust terminal workspace that runs as the `muffin` command.

It gives you four panes inside a project directory:

- Files
- File Viewer / Diff Viewer
- Terminal
- Codex

It starts in the current working directory and uses that directory as:

- the root of the file tree
- the working directory for shell commands
- the working directory for the embedded `codex` session

## Requirements

- Rust and Cargo
- `codex` installed and available on `PATH`

## Setup Prerequisites

### 1. Install Rust and Cargo

If `rustc` and `cargo` are not installed yet, install them with `rustup`:

```bash
curl https://sh.rustup.rs -sSf | sh
```

After installation, restart your shell or load Cargo's environment:

```bash
source "$HOME/.cargo/env"
```

Verify the installation:

```bash
rustc --version
cargo --version
```

### 2. Install and authenticate Codex CLI

`muffin` launches the `codex` command inside the Codex pane, so the CLI must already be installed and authenticated on your machine.

Verify that the command is available:

```bash
codex --version
```

If you still need to authenticate, run:

```bash
codex login
```

Then confirm the CLI is ready before starting `muffin`.

### 3. Sanity check

Before installing or running `muffin`, this should work:

```bash
cargo --version
codex --version
```

## Install

Install from crates.io:

```bash
cargo install muffintui
```

This installs the executable as:

```bash
muffin
```

Install from the local checkout:

```bash
cargo install --path .
```

That local install also provides the `muffin` executable.

## Run

Launch in the current directory:

```bash
muffin
```

Launch against another project:

```bash
cd /path/to/project
muffin
```

Run without installing during local development:

```bash
cargo run
```

## What It Does

- Shows a navigable file tree rooted at the current directory
- Opens the selected file in a read-only file viewer
- Highlights source code in normal file view with theme-aware colors
- Toggles a diff viewer against `HEAD~1`
- Runs shell commands inside the built-in terminal pane with `sh -lc`
- Embeds a live `codex` terminal session in the right pane
- Cycles between three built-in themes
- Ships with integration tests under `tests/`

Notes:

- `.git` and `target` are intentionally hidden from the file tree
- The built-in terminal pane starts empty
- Diff mode falls back to a message when the repository has no `HEAD~1`
- If the initial `codex` launch fails, pressing `Enter` in the Codex pane retries the session
- If `codex` is not installed, the rest of the TUI still works and the Codex pane shows the startup error

## Keybindings

### Global

- `Tab`: move focus to the next pane
- `Shift+Tab`: cycle the theme
- `Esc`: quit
- `Ctrl+C`: quit when focus is not in the Codex pane

### Files Pane

- `Up` or `k`: move selection up
- `Down` or `j`: move selection down
- `Enter` on a directory: expand or collapse it
- `Enter` on a file: open it in the file viewer

### File Viewer / Diff Viewer

- `Ctrl+D`: toggle between file view and diff view
- `PageUp`: scroll up
- `PageDown`: scroll down

### Terminal Pane

- Type directly into the prompt
- `Enter`: run the current command
- `Backspace`: delete one character
- `PageUp`: scroll back
- `PageDown`: scroll forward
- `Home`: jump to the oldest visible terminal history
- `End`: jump back to the live prompt

### Codex Pane

- Regular typing: send input to the embedded `codex` session
- `Enter`: submit input, or retry the session if startup failed
- `Ctrl+C`: send interrupt to `codex`
- `Arrow keys`, `PageUp`, `PageDown`, `Home`, `End`, `Tab`, `Backspace`: forwarded to the embedded session

## Publish

Before publishing:

```bash
cargo package
```

Then publish:

```bash
cargo publish
```

## Test

Run the integration test suite with:

```bash
cargo test
```
