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

#[cfg(test)]
mod tests {
    use super::{collapse_directory, collect_visible_file_entries};
    use std::{
        collections::HashSet,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_test_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("muffin-{name}-{nanos}"))
    }

    #[test]
    fn collects_sorted_entries_and_hides_git_and_target() {
        let root = temp_test_dir("file-tree");
        fs::create_dir_all(root.join("b_dir")).unwrap();
        fs::create_dir_all(root.join("a_dir")).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(root.join("z.txt"), "z").unwrap();
        fs::write(root.join("a.txt"), "a").unwrap();

        let entries = collect_visible_file_entries(&root, &HashSet::new()).unwrap();
        let labels: Vec<_> = entries.iter().map(|entry| entry.display.as_str()).collect();

        assert_eq!(labels, vec!["▸ a_dir/", "▸ b_dir/", "  a.txt", "  z.txt"]);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn expands_directories_and_tracks_depth() {
        let root = temp_test_dir("expanded-tree");
        fs::create_dir_all(root.join("dir").join("nested")).unwrap();
        fs::write(root.join("dir").join("nested").join("file.txt"), "hello").unwrap();

        let mut expanded = HashSet::new();
        expanded.insert(root.join("dir"));
        expanded.insert(root.join("dir").join("nested"));

        let entries = collect_visible_file_entries(&root, &expanded).unwrap();
        assert_eq!(entries[0].display, "▾ dir/");
        assert_eq!(entries[0].depth, 0);
        assert_eq!(entries[1].display, "▾ nested/");
        assert_eq!(entries[1].depth, 1);
        assert_eq!(entries[2].display, "  file.txt");
        assert_eq!(entries[2].depth, 2);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn collapse_directory_removes_descendants() {
        let root = PathBuf::from("/tmp/root");
        let mut expanded = HashSet::from([
            root.join("dir"),
            root.join("dir").join("child"),
            root.join("other"),
        ]);

        collapse_directory(&root.join("dir"), &mut expanded);

        assert!(!expanded.contains(&root.join("dir")));
        assert!(!expanded.contains(&root.join("dir").join("child")));
        assert!(expanded.contains(&root.join("other")));
    }
}
