use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use crate::Fs;
use crate::osvfs;

use super::FileWatcher;

fn default_paths(root: &str) -> Vec<String> {
    [
        "src/a.ts",
        "src/b.ts",
        "src/c.ts",
        "src/sub/d.ts",
        "tsconfig.json",
    ]
    .into_iter()
    .map(|path| format!("{root}/{path}"))
    .collect()
}

fn new_test_fs() -> (String, Arc<dyn Fs + Send + Sync>) {
    let root = std::env::temp_dir().join(format!(
        "tsgo-vfswatch-race-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let src = root.join("src");
    let sub = src.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(src.join("a.ts"), "const a = 1;").unwrap();
    fs::write(src.join("b.ts"), "const b = 2;").unwrap();
    fs::write(src.join("c.ts"), "const c = 3;").unwrap();
    fs::write(sub.join("d.ts"), "const d = 4;").unwrap();
    fs::write(root.join("tsconfig.json"), "{}").unwrap();

    (
        root.to_string_lossy().into_owned(),
        Arc::new(osvfs::os::fs()),
    )
}

fn new_watcher_with_state(fs: Arc<dyn Fs + Send + Sync>, root: &str) -> Arc<FileWatcher> {
    let fw = Arc::new(FileWatcher::new(fs, Duration::from_millis(10), true, || {}));
    fw.update_watch_state(&default_paths(root), &BTreeMap::new());
    fw
}

#[test]
fn test_race_has_changes_vs_update_watch_state() {
    let (root, fs) = new_test_fs();
    let fw = new_watcher_with_state(fs, &root);
    let mut handles = Vec::new();

    for _ in 0..10 {
        let fw = fw.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..200 {
                fw.has_changes_from_watch_state();
            }
        }));
    }

    for _ in 0..5 {
        let fw = fw.clone();
        let paths = vec![format!("{root}/src/a.ts"), format!("{root}/src/b.ts")];
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                fw.update_watch_state(&paths, &BTreeMap::new());
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_race_wildcard_directories_access() {
    let (root, fs) = new_test_fs();
    let fw = new_watcher_with_state(fs, &root);
    let wildcard_dirs = BTreeMap::from([(format!("{root}/src"), true)]);
    fw.update_watch_state(&default_paths(&root), &wildcard_dirs);
    let mut handles = Vec::new();

    for _ in 0..10 {
        let fw = fw.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..200 {
                fw.has_changes_from_watch_state();
            }
        }));
    }

    for _ in 0..5 {
        let fw = fw.clone();
        let paths = default_paths(&root);
        let wildcard_dirs = wildcard_dirs.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                fw.update_watch_state(&paths, &wildcard_dirs);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_race_poll_interval_access() {
    let (root, fs) = new_test_fs();
    let fw = new_watcher_with_state(fs, &root);
    let mut handles = Vec::new();

    for _ in 0..10 {
        let fw = fw.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..500 {
                fw.has_changes_from_watch_state();
            }
        }));
    }

    for i in 0..5 {
        let fw = fw.clone();
        handles.push(thread::spawn(move || {
            for j in 0..200 {
                fw.set_poll_interval(Duration::from_millis(i * 200 + j));
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_race_mixed_operations() {
    let (root, fs) = new_test_fs();
    let fw = new_watcher_with_state(fs.clone(), &root);
    let wildcard_dirs = BTreeMap::from([(format!("{root}/src"), true)]);
    fw.update_watch_state(&default_paths(&root), &wildcard_dirs);
    let mut handles = Vec::new();

    for _ in 0..8 {
        let fw = fw.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                fw.has_changes_from_watch_state();
            }
        }));
    }

    for i in 0..4 {
        let fw = fw.clone();
        let wildcard_dirs = wildcard_dirs.clone();
        let root = root.clone();
        handles.push(thread::spawn(move || {
            for j in 0..50 {
                let paths = vec![
                    format!("{root}/src/a.ts"),
                    format!("{root}/src/new_{i}_{j}.ts"),
                ];
                fw.update_watch_state(&paths, &wildcard_dirs);
            }
        }));
    }

    for i in 0..4 {
        let fs = fs.clone();
        let root = root.clone();
        handles.push(thread::spawn(move || {
            for j in 0..50 {
                let path = format!("{root}/src/gen_{i}_{j}.ts");
                let _ = fs.write_file(&path, &format!("const x = {j};"));
                if j % 3 == 0 {
                    let _ = fs.remove(&path);
                }
            }
        }));
    }

    for _ in 0..2 {
        let fw = fw.clone();
        handles.push(thread::spawn(move || {
            for j in 0..100 {
                fw.set_poll_interval(Duration::from_millis(50 + j));
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_race_update_with_concurrent_file_modifications() {
    let (root, fs) = new_test_fs();
    let fw = new_watcher_with_state(fs.clone(), &root);
    let wildcard_dirs = BTreeMap::from([(format!("{root}/src"), true)]);
    fw.update_watch_state(&default_paths(&root), &wildcard_dirs);
    let mut handles = Vec::new();

    for i in 0..6 {
        let fs = fs.clone();
        let root = root.clone();
        handles.push(thread::spawn(move || {
            for j in 0..100 {
                let path = format!("{root}/src/churn_{i}_{j}.ts");
                let _ = fs.write_file(&path, &format!("export const v = {j};"));
                let _ = fs.remove(&path);
            }
        }));
    }

    for _ in 0..4 {
        let fw = fw.clone();
        let wildcard_dirs = wildcard_dirs.clone();
        let paths = vec![format!("{root}/src/a.ts"), format!("{root}/tsconfig.json")];
        handles.push(thread::spawn(move || {
            for _ in 0..50 {
                fw.update_watch_state(&paths, &wildcard_dirs);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
    let _ = fs::remove_dir_all(root);
}

fn run_file_watcher_operations(ops: &[u8]) {
    if ops.is_empty() {
        return;
    }

    let (root, fs) = new_test_fs();
    let fw = new_watcher_with_state(fs.clone(), &root);
    let files = [
        format!("{root}/src/a.ts"),
        format!("{root}/src/b.ts"),
        format!("{root}/src/c.ts"),
        format!("{root}/src/new.ts"),
        format!("{root}/src/sub/new.ts"),
    ];

    for (i, op) in ops.iter().enumerate() {
        let path = &files[i % files.len()];
        match op % 6 {
            0 => {
                let _ = fs.write_file(path, &format!("const x = {i};"));
            }
            1 => {
                let _ = fs.remove(path);
            }
            2 => {
                fw.has_changes_from_watch_state();
            }
            3 => {
                fw.update_watch_state(&files, &BTreeMap::new());
            }
            4 => {
                fw.update_watch_state(&files, &BTreeMap::from([(format!("{root}/src"), true)]));
                fw.has_changes_from_watch_state();
            }
            5 => {
                fw.set_poll_interval(Duration::from_millis((i * 10) as u64));
            }
            _ => unreachable!(),
        }
    }
    let _ = fs::remove_dir_all(root);
}

// PORT NOTE: Go fuzz targets are represented as deterministic seed tests until
// the Rust test harness has native fuzzing wired into this port.
#[test]
fn fuzz_file_watcher_operations_seeds() {
    for ops in [
        &[0, 1, 2, 3, 0, 1, 2, 3][..],
        &[2, 2, 2, 0, 0, 1, 3, 3],
        &[3, 3, 3, 3, 0, 0, 0, 0],
        &[4, 4, 4, 0, 2, 1, 3, 2],
        &[5, 5, 5, 5, 5, 5, 5, 5],
        &[0, 0, 0, 0, 0, 0, 0, 0],
        &[1, 1, 1, 1, 1, 1, 1, 1],
    ] {
        run_file_watcher_operations(ops);
    }
}

// PORT NOTE: Go fuzz targets are represented as deterministic seed tests until
// the Rust test harness has native fuzzing wired into this port.
#[test]
fn fuzz_file_watcher_concurrent_seeds() {
    for ops in [
        &[0, 1, 2, 3, 4, 5, 0, 1, 2, 3, 4, 5][..],
        &[0, 0, 0, 3, 3, 3, 2, 2, 2, 1, 1, 1],
        &[2, 3, 2, 3, 2, 3, 0, 0, 0, 0, 0, 0],
    ] {
        if ops.len() < 4 {
            continue;
        }

        let (root, fs) = new_test_fs();
        let fw = new_watcher_with_state(fs.clone(), &root);
        fw.update_watch_state(
            &default_paths(&root),
            &BTreeMap::from([(format!("{root}/src"), true)]),
        );
        let files = [
            format!("{root}/src/a.ts"),
            format!("{root}/src/b.ts"),
            format!("{root}/src/c.ts"),
            format!("{root}/src/new.ts"),
        ];
        let chunk_size = (ops.len() / 2).max(1);
        let mut handles = Vec::new();

        for (goroutine_id, chunk) in ops.chunks(chunk_size).enumerate() {
            let fs = fs.clone();
            let fw = fw.clone();
            let root = root.clone();
            let files = files.clone();
            let chunk = chunk.to_vec();
            handles.push(thread::spawn(move || {
                for (i, op) in chunk.iter().enumerate() {
                    let path = &files[(goroutine_id * chunk.len() + i) % files.len()];
                    match op % 4 {
                        0 => {
                            let _ = fs.write_file(path, &format!("const g{goroutine_id} = {i};"));
                        }
                        1 => {
                            let _ = fs.remove(path);
                        }
                        2 => {
                            fw.has_changes_from_watch_state();
                        }
                        3 => {
                            fw.update_watch_state(
                                &[path.clone()],
                                &BTreeMap::from([(format!("{root}/src"), true)]),
                            );
                        }
                        _ => unreachable!(),
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
        let _ = fs::remove_dir_all(root);
    }
}
