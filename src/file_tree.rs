use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub display: String,
    pub is_dir: bool,
    pub depth: usize,
    pub is_updated: bool,
}

pub fn collect_visible_file_entries(
    root: &Path,
    expanded_dirs: &HashSet<PathBuf>,
) -> io::Result<Vec<FileEntry>> {
    let updated_paths = collect_updated_paths(root).unwrap_or_default();
    let mut out = Vec::new();
    collect_visible_file_entries_recursive(root, root, expanded_dirs, &updated_paths, 0, &mut out)?;
    Ok(out)
}

pub fn collapse_directory(path: &Path, expanded_dirs: &mut HashSet<PathBuf>) {
    expanded_dirs.retain(|candidate| candidate != path && !candidate.starts_with(path));
}

fn collect_visible_file_entries_recursive(
    root: &Path,
    dir: &Path,
    expanded_dirs: &HashSet<PathBuf>,
    updated_paths: &HashSet<PathBuf>,
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
        let is_updated = if is_dir {
            updated_paths
                .iter()
                .any(|candidate| candidate.starts_with(&path))
        } else {
            updated_paths.contains(&path)
        };
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
            is_updated,
        });

        if is_dir && expanded_dirs.contains(&path) {
            collect_visible_file_entries_recursive(
                root,
                &path,
                expanded_dirs,
                updated_paths,
                depth + 1,
                out,
            )?;
        }
    }

    if depth == 0 && out.is_empty() {
        out.push(FileEntry {
            path: root.to_path_buf(),
            display: "(empty)".to_string(),
            is_dir: false,
            depth: 0,
            is_updated: false,
        });
    }

    Ok(())
}

fn collect_updated_paths(root: &Path) -> io::Result<HashSet<PathBuf>> {
    let output = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .arg("--untracked-files=normal")
        .current_dir(root)
        .output()?;

    if !output.status.success() {
        return Ok(HashSet::new());
    }

    let mut updated = HashSet::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.len() < 4 {
            continue;
        }

        let raw_path = &line[3..];
        let path_str = raw_path
            .rsplit_once(" -> ")
            .map(|(_, path)| path)
            .unwrap_or(raw_path)
            .trim_matches('"');

        if path_str.is_empty() {
            continue;
        }

        updated.insert(root.join(path_str));
    }

    Ok(updated)
}
