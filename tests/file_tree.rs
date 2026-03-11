use muffintui::file_tree::{collapse_directory, collect_visible_file_entries};
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
