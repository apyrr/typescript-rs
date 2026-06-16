use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use ts_vfs::Fs;
use ts_vfs::vfstest::IntoMapFile;

use crate::tsc;

use super::FileMap;
use super::runner::TscInput;
use super::sys::{TestSys, new_test_sys};

// PORT NOTE: Go shares *execute.Watcher directly across goroutines. The current
// Rust watcher trait exposes do_cycle as &mut self, so the test keeps the same
// thread topology and concurrent file-system mutations, but serializes entry
// through this wrapper until the concrete watcher API can be shared directly.
struct SharedWatcher {
    watcher: Mutex<Box<dyn tsc::Watcher>>,
}

impl SharedWatcher {
    fn new(watcher: Box<dyn tsc::Watcher>) -> Self {
        Self {
            watcher: Mutex::new(watcher),
        }
    }

    fn do_cycle(&self) {
        self.watcher
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .do_cycle();
    }
}

// createTestWatcher sets up a minimal project with a tsconfig and returns a
// Watcher ready for concurrent testing, plus the TestSys for file manipulation.
fn create_test_watcher() -> (Arc<SharedWatcher>, TestSys) {
    let input = TscInput {
        files: file_map(&[
            ("/home/src/workspaces/project/a.ts", "const a: number = 1;"),
            (
                "/home/src/workspaces/project/b.ts",
                "import { a } from \"./a\"; export const b = a;",
            ),
            ("/home/src/workspaces/project/tsconfig.json", "{}"),
        ]),
        command_line_args: vec!["--watch".to_owned()],
        sub_scenario: String::new(),
        cwd: String::new(),
        edits: Vec::new(),
        env: HashMap::new(),
        ignore_case: false,
        windows_style_root: String::new(),
    };
    let sys = new_test_sys(&input, false);
    let result = crate::command_line(
        sys.clone_system(),
        vec!["--watch".to_owned()],
        Some(sys.clone_testing()),
    );
    let watcher = result
        .watcher
        .unwrap_or_else(|| panic!("expected Watcher to be non-nil in watch mode"));
    (Arc::new(SharedWatcher::new(watcher)), sys)
}

fn file_map(files: &[(&str, &str)]) -> FileMap {
    files
        .iter()
        .map(|(path, content)| {
            (
                (*path).to_owned(),
                (*content).into_map_file(SystemTime::UNIX_EPOCH),
            )
        })
        .collect()
}

// TestWatcherConcurrentDoCycle calls DoCycle from multiple goroutines while
// modifying source files, exposing data races on Watcher fields such as
// configModified, program, config, and the underlying FileWatcher state. Run
// with -race to detect.
#[test]
fn test_watcher_concurrent_do_cycle() {
    let (w, sys) = create_test_watcher();

    std::thread::scope(|scope| {
        for i in 0..8 {
            let w = Arc::clone(&w);
            let sys = sys.clone();
            scope.spawn(move || {
                for j in 0..10 {
                    let _ = sys.fs_from_file_map().write_file(
                        "/home/src/workspaces/project/a.ts",
                        &format!("const a: number = {};", i * 10 + j),
                    );
                    w.do_cycle();
                }
            });
        }
    });
}

// TestWatcherDoCycleWithConcurrentStateReads calls DoCycle from multiple
// goroutines, some modifying files and some not, to test concurrent access to
// all Watcher and FileWatcher state.
#[test]
fn test_watcher_do_cycle_with_concurrent_state_reads() {
    let (w, sys) = create_test_watcher();

    std::thread::scope(|scope| {
        // DoCycle goroutines
        for i in 0..4 {
            let w = Arc::clone(&w);
            let sys = sys.clone();
            scope.spawn(move || {
                for j in 0..15 {
                    let _ = sys.fs_from_file_map().write_file(
                        "/home/src/workspaces/project/a.ts",
                        &format!("const a: number = {};", i * 15 + j),
                    );
                    w.do_cycle();
                }
            });
        }

        // State reader goroutines
        for _ in 0..8 {
            let w = Arc::clone(&w);
            scope.spawn(move || {
                for _ in 0..50 {
                    w.do_cycle();
                    w.do_cycle();
                    w.do_cycle();
                    w.do_cycle();
                }
            });
        }
    });
}

// TestWatcherConcurrentFileChangesAndDoCycle creates, modifies, and deletes
// files from multiple goroutines while DoCycle runs, testing races between FS
// mutations and watch state updates.
#[test]
fn test_watcher_concurrent_file_changes_and_do_cycle() {
    let (w, sys) = create_test_watcher();

    std::thread::scope(|scope| {
        // File creators
        for i in 0..4 {
            let sys = sys.clone();
            scope.spawn(move || {
                for j in 0..20 {
                    let path = format!("/home/src/workspaces/project/gen_{i}_{j}.ts");
                    let _ = sys
                        .fs_from_file_map()
                        .write_file(&path, &format!("export const x{i}_{j} = {j};"));
                }
            });
        }

        // File deleters
        let sys_for_deleter = sys.clone();
        scope.spawn(move || {
            for j in 0..20 {
                let _ = sys_for_deleter
                    .fs_from_file_map()
                    .remove(&format!("/home/src/workspaces/project/gen_0_{j}.ts"));
            }
        });

        // DoCycle callers
        for _ in 0..4 {
            let w = Arc::clone(&w);
            scope.spawn(move || {
                for _ in 0..10 {
                    w.do_cycle();
                }
            });
        }
    });
}

// TestWatcherRapidConfigChanges modifies tsconfig.json rapidly from multiple
// goroutines while DoCycle runs, testing races on config-related fields
// (configModified, configHasErrors, configFilePaths, config,
// extendedConfigCache).
#[test]
fn test_watcher_rapid_config_changes() {
    let (w, sys) = create_test_watcher();
    let configs = Arc::new([
        "{}",
        "{\"compilerOptions\": {\"strict\": true}}",
        "{\"compilerOptions\": {\"target\": \"ES2020\"}}",
        "{\"compilerOptions\": {\"noEmit\": true}}",
    ]);

    std::thread::scope(|scope| {
        // Config modifiers + DoCycle
        for i in 0..3 {
            let configs = Arc::clone(&configs);
            let w = Arc::clone(&w);
            let sys = sys.clone();
            scope.spawn(move || {
                for j in 0..10 {
                    let _ = sys.fs_from_file_map().write_file(
                        "/home/src/workspaces/project/tsconfig.json",
                        configs[(i + j) % configs.len()],
                    );
                    w.do_cycle();
                }
            });
        }

        // Concurrent source file modifications
        for i in 0..2 {
            let w = Arc::clone(&w);
            let sys = sys.clone();
            scope.spawn(move || {
                for j in 0..15 {
                    let _ = sys.fs_from_file_map().write_file(
                        "/home/src/workspaces/project/a.ts",
                        &format!("const a: number = {};", i * 15 + j),
                    );
                    w.do_cycle();
                }
            });
        }

        // State readers
        for _ in 0..4 {
            let w = Arc::clone(&w);
            scope.spawn(move || {
                for _ in 0..30 {
                    w.do_cycle();
                    w.do_cycle();
                }
            });
        }
    });
}

// TestWatcherConcurrentDoCycleNoChanges calls DoCycle from many goroutines when
// no files have changed, testing the early-return path where WatchState is read
// and HasChanges is called.
#[test]
fn test_watcher_concurrent_do_cycle_no_changes() {
    let (w, _) = create_test_watcher();

    std::thread::scope(|scope| {
        for _ in 0..16 {
            let w = Arc::clone(&w);
            scope.spawn(move || {
                for _ in 0..50 {
                    w.do_cycle();
                }
            });
        }
    });
}

// TestWatcherAlternatingModifyAndDoCycle alternates between modifying a file
// and calling DoCycle from different goroutines, creating a realistic scenario
// where the file watcher detects changes mid-cycle.
#[test]
fn test_watcher_alternating_modify_and_do_cycle() {
    let (w, sys) = create_test_watcher();

    std::thread::scope(|scope| {
        // Writer goroutine: continuously modifies files
        let sys_for_writer = sys.clone();
        scope.spawn(move || {
            for j in 0..100 {
                let _ = sys_for_writer.fs_from_file_map().write_file(
                    "/home/src/workspaces/project/a.ts",
                    &format!("const a: number = {j};"),
                );
            }
        });

        // Multiple DoCycle goroutines
        for _ in 0..4 {
            let w = Arc::clone(&w);
            scope.spawn(move || {
                for _ in 0..25 {
                    w.do_cycle();
                }
            });
        }

        // State reader goroutines
        for _ in 0..4 {
            let w = Arc::clone(&w);
            scope.spawn(move || {
                for _ in 0..100 {
                    w.do_cycle();
                }
            });
        }
    });
}
