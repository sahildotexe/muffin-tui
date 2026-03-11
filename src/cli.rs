use std::io;

use crate::codex::SessionMode;

pub fn parse_session_mode(args: impl Iterator<Item = String>) -> io::Result<SessionMode> {
    let mut mode = SessionMode::Shell;

    for arg in args {
        match arg.as_str() {
            "--codex" => {
                if mode != SessionMode::Shell {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "choose only one of --codex or --claude",
                    ));
                }
                mode = SessionMode::Codex;
            }
            "--claude" => {
                if mode != SessionMode::Shell {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "choose only one of --codex or --claude",
                    ));
                }
                mode = SessionMode::Claude;
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument: {arg}"),
                ));
            }
        }
    }

    Ok(mode)
}
