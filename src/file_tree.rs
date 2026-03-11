use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub display: String,
    pub is_dir: bool,
    pub depth: usize,
}

pub fn collect_visible_file_entries(
    root: &Path,
    expanded_dirs: &HashSet<PathBuf>,
) -> io::Result<Vec<FileEntry>> {
    let mut out = Vec::new();
    collect_visible_file_entries_recursive(root, root, expanded_dirs, 0, &mut out)?;
    Ok(out)
}

pub fn collapse_directory(path: &Path, expanded_dirs: &mut HashSet<PathBuf>) {
    expanded_dirs.retain(|candidate| candidate != path && !candidate.starts_with(path));
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
