use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use ts_collections as collections;
use ts_lsproto as lsproto;
use ts_tspath as tspath;
use ts_vfs::{vfs::Fs, vfstest};

use crate::{
    FileChangeSummary,
    autoimport::AutoImportBuilderFs,
    overlayfs::{FileContent, new_disk_file, new_overlay},
    snapshotfs::{MemoizedDiskFile, SnapshotFs, new_snapshot_fs_builder, new_source_fs},
};

fn to_path(file_name: &str) -> tspath::Path {
    file_name.into()
}

fn to_path_mapper() -> Arc<dyn Fn(&str) -> tspath::Path + Send + Sync> {
    Arc::new(to_path)
}

fn empty_read_files() -> Mutex<HashMap<tspath::Path, Arc<MemoizedDiskFile>>> {
    Mutex::new(HashMap::new())
}

fn map_file(content: &str) -> vfstest::MapFile {
    vfstest::MapFile {
        data: std::sync::Arc::from(content.as_bytes()),
        ..Default::default()
    }
}

#[test]
fn test_snapshot_fs_builder() {
    // builds directory tree on file add
    {
        let test_fs = vfstest::from_map(HashMap::from([("/src/foo.ts", "const foo = 1;")]), false);
        let mut builder = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );

        let fh = builder.get_file("/src/foo.ts");
        assert!(fh.is_some(), "file should exist");
        assert_eq!(fh.unwrap().content(), "const foo = 1;");

        let (snapshot, changed) = builder.finalize();
        assert!(changed, "should have changed");
        assert!(
            snapshot.disk_directories[&tspath::Path::from("/src")]
                .contains_key(&tspath::Path::from("/src/foo.ts")),
            "/src should contain /src/foo.ts"
        );
        assert!(
            snapshot.disk_directories[&tspath::Path::from("/")]
                .contains_key(&tspath::Path::from("/src")),
            "/ should contain /src"
        );
    }

    // builds nested directory tree
    {
        let test_fs = vfstest::from_map(
            HashMap::from([("/src/nested/deep/file.ts", "export const x = 1;")]),
            false,
        );
        let mut builder = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );

        assert!(builder.get_file("/src/nested/deep/file.ts").is_some());
        let (snapshot, changed) = builder.finalize();
        assert!(changed, "should have changed");
        assert!(
            snapshot.disk_directories[&tspath::Path::from("/src/nested/deep")]
                .contains_key(&tspath::Path::from("/src/nested/deep/file.ts"))
        );
        assert!(
            snapshot.disk_directories[&tspath::Path::from("/src/nested")]
                .contains_key(&tspath::Path::from("/src/nested/deep"))
        );
        assert!(
            snapshot.disk_directories[&tspath::Path::from("/src")]
                .contains_key(&tspath::Path::from("/src/nested"))
        );
        assert!(
            snapshot.disk_directories[&tspath::Path::from("/")]
                .contains_key(&tspath::Path::from("/src"))
        );
    }

    // removes directory entries on file delete
    {
        let test_fs = vfstest::from_map(HashMap::from([("/src/foo.ts", "const foo = 1;")]), false);
        let existing_disk_files = HashMap::from([(
            tspath::Path::from("/src/foo.ts"),
            Arc::new(new_disk_file(
                "/src/foo.ts".to_string(),
                "const foo = 1;".to_string(),
            )),
        )]);
        let existing_dirs = HashMap::from([
            (
                tspath::Path::from("/"),
                HashMap::from([(tspath::Path::from("/src"), "src".to_string())]),
            ),
            (
                tspath::Path::from("/src"),
                HashMap::from([(tspath::Path::from("/src/foo.ts"), "foo.ts".to_string())]),
            ),
        ]);
        let builder = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            HashMap::new(),
            existing_disk_files,
            existing_dirs,
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );

        if let Some(entry) = builder.disk_files.load(&tspath::Path::from("/src/foo.ts")) {
            entry.delete();
        }

        let (snapshot, changed) = builder.finalize();
        assert!(changed, "should have changed");
        assert!(
            !snapshot
                .disk_files
                .contains_key(&tspath::Path::from("/src/foo.ts"))
        );
        assert!(
            !snapshot
                .disk_directories
                .contains_key(&tspath::Path::from("/src"))
        );
        assert!(
            !snapshot
                .disk_directories
                .contains_key(&tspath::Path::from("/"))
        );
    }

    // removes only empty directories on file delete
    {
        let test_fs = vfstest::from_map(
            HashMap::from([
                ("/src/foo.ts", "const foo = 1;"),
                ("/src/bar.ts", "const bar = 2;"),
            ]),
            false,
        );
        let existing_disk_files = HashMap::from([
            (
                tspath::Path::from("/src/foo.ts"),
                Arc::new(new_disk_file(
                    "/src/foo.ts".to_string(),
                    "const foo = 1;".to_string(),
                )),
            ),
            (
                tspath::Path::from("/src/bar.ts"),
                Arc::new(new_disk_file(
                    "/src/bar.ts".to_string(),
                    "const bar = 2;".to_string(),
                )),
            ),
        ]);
        let existing_dirs = HashMap::from([
            (
                tspath::Path::from("/"),
                HashMap::from([(tspath::Path::from("/src"), "src".to_string())]),
            ),
            (
                tspath::Path::from("/src"),
                HashMap::from([
                    (tspath::Path::from("/src/foo.ts"), "foo.ts".to_string()),
                    (tspath::Path::from("/src/bar.ts"), "bar.ts".to_string()),
                ]),
            ),
        ]);
        let builder = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            HashMap::new(),
            existing_disk_files,
            existing_dirs,
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );

        if let Some(entry) = builder.disk_files.load(&tspath::Path::from("/src/foo.ts")) {
            entry.delete();
        }

        let (snapshot, changed) = builder.finalize();
        assert!(changed, "should have changed");
        assert!(
            !snapshot
                .disk_files
                .contains_key(&tspath::Path::from("/src/foo.ts"))
        );
        assert!(
            snapshot
                .disk_files
                .contains_key(&tspath::Path::from("/src/bar.ts"))
        );
        let src_dir = &snapshot.disk_directories[&tspath::Path::from("/src")];
        assert!(!src_dir.contains_key(&tspath::Path::from("/src/foo.ts")));
        assert!(src_dir.contains_key(&tspath::Path::from("/src/bar.ts")));
        assert!(
            snapshot.disk_directories[&tspath::Path::from("/")]
                .contains_key(&tspath::Path::from("/src"))
        );
    }

    // adds file to existing directory
    {
        let test_fs = vfstest::from_map(
            HashMap::from([
                ("/src/foo.ts", "const foo = 1;"),
                ("/src/bar.ts", "const bar = 2;"),
            ]),
            false,
        );
        let existing_disk_files = HashMap::from([(
            tspath::Path::from("/src/foo.ts"),
            Arc::new(new_disk_file(
                "/src/foo.ts".to_string(),
                "const foo = 1;".to_string(),
            )),
        )]);
        let existing_dirs = HashMap::from([
            (
                tspath::Path::from("/"),
                HashMap::from([(tspath::Path::from("/src"), "src".to_string())]),
            ),
            (
                tspath::Path::from("/src"),
                HashMap::from([(tspath::Path::from("/src/foo.ts"), "foo.ts".to_string())]),
            ),
        ]);
        let mut builder = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            HashMap::new(),
            existing_disk_files,
            existing_dirs,
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );

        assert!(builder.get_file("/src/bar.ts").is_some());
        let (snapshot, changed) = builder.finalize();
        assert!(changed, "should have changed");
        let src_dir = &snapshot.disk_directories[&tspath::Path::from("/src")];
        assert!(src_dir.contains_key(&tspath::Path::from("/src/foo.ts")));
        assert!(src_dir.contains_key(&tspath::Path::from("/src/bar.ts")));
    }

    // no change when no files added or deleted
    {
        let test_fs = vfstest::from_map(HashMap::from([("/src/foo.ts", "const foo = 1;")]), false);
        let existing_disk_files = HashMap::from([(
            tspath::Path::from("/src/foo.ts"),
            Arc::new(new_disk_file(
                "/src/foo.ts".to_string(),
                "const foo = 1;".to_string(),
            )),
        )]);
        let existing_dirs = HashMap::from([
            (
                tspath::Path::from("/"),
                HashMap::from([(tspath::Path::from("/src"), "src".to_string())]),
            ),
            (
                tspath::Path::from("/src"),
                HashMap::from([(tspath::Path::from("/src/foo.ts"), "foo.ts".to_string())]),
            ),
        ]);
        let builder = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            HashMap::new(),
            existing_disk_files,
            existing_dirs,
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );

        let (snapshot, changed) = builder.finalize();
        assert!(!changed, "should not have changed");
        assert!(
            snapshot.disk_directories[&tspath::Path::from("/src")]
                .contains_key(&tspath::Path::from("/src/foo.ts"))
        );
    }

    // overlay files are returned over disk files
    {
        let test_fs = vfstest::from_map(HashMap::from([("/src/foo.ts", "const foo = 1;")]), false);
        let overlays = HashMap::from([(
            tspath::Path::from("/src/foo.ts"),
            Arc::new(new_overlay(
                "/src/foo.ts".to_string(),
                "const foo = 999;".to_string(),
                0,
                ts_core::ScriptKind::TS,
            )),
        )]);
        let mut builder = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            overlays,
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );

        let fh = builder.get_file("/src/foo.ts");
        assert!(fh.is_some());
        assert_eq!(fh.unwrap().content(), "const foo = 999;");
    }

    // multiple files added and deleted in single cycle
    {
        let test_fs = vfstest::from_map(
            HashMap::from([
                ("/src/a.ts", "const a = 1;"),
                ("/src/b.ts", "const b = 2;"),
                ("/lib/utils.ts", "export const util = 1;"),
                ("/lib/helpers.ts", "export const helper = 1;"),
                ("/other/single.ts", "const single = 1;"),
            ]),
            false,
        );
        let existing_disk_files = HashMap::from([
            (
                tspath::Path::from("/src/a.ts"),
                Arc::new(new_disk_file(
                    "/src/a.ts".to_string(),
                    "const a = 1;".to_string(),
                )),
            ),
            (
                tspath::Path::from("/other/single.ts"),
                Arc::new(new_disk_file(
                    "/other/single.ts".to_string(),
                    "const single = 1;".to_string(),
                )),
            ),
        ]);
        let existing_dirs = HashMap::from([
            (
                tspath::Path::from("/"),
                HashMap::from([
                    (tspath::Path::from("/src"), "src".to_string()),
                    (tspath::Path::from("/other"), "other".to_string()),
                ]),
            ),
            (
                tspath::Path::from("/src"),
                HashMap::from([(tspath::Path::from("/src/a.ts"), "a.ts".to_string())]),
            ),
            (
                tspath::Path::from("/other"),
                HashMap::from([(
                    tspath::Path::from("/other/single.ts"),
                    "single.ts".to_string(),
                )]),
            ),
        ]);
        let mut builder = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            HashMap::new(),
            existing_disk_files,
            existing_dirs,
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );

        assert!(builder.get_file("/src/b.ts").is_some());
        assert!(builder.get_file("/lib/utils.ts").is_some());
        assert!(builder.get_file("/lib/helpers.ts").is_some());
        if let Some(entry) = builder.disk_files.load(&tspath::Path::from("/src/a.ts")) {
            entry.delete();
        }
        if let Some(entry) = builder
            .disk_files
            .load(&tspath::Path::from("/other/single.ts"))
        {
            entry.delete();
        }

        let (snapshot, changed) = builder.finalize();
        assert!(changed, "should have changed");
        assert!(
            !snapshot
                .disk_files
                .contains_key(&tspath::Path::from("/src/a.ts"))
        );
        assert!(
            !snapshot
                .disk_files
                .contains_key(&tspath::Path::from("/other/single.ts"))
        );
        assert!(
            snapshot
                .disk_files
                .contains_key(&tspath::Path::from("/src/b.ts"))
        );
        assert!(
            snapshot
                .disk_files
                .contains_key(&tspath::Path::from("/lib/utils.ts"))
        );
        assert!(
            snapshot
                .disk_files
                .contains_key(&tspath::Path::from("/lib/helpers.ts"))
        );
        assert!(
            !snapshot
                .disk_directories
                .contains_key(&tspath::Path::from("/other"))
        );
        assert!(
            snapshot.disk_directories[&tspath::Path::from("/src")]
                .contains_key(&tspath::Path::from("/src/b.ts"))
        );
        assert!(
            !snapshot.disk_directories[&tspath::Path::from("/src")]
                .contains_key(&tspath::Path::from("/src/a.ts"))
        );
        assert!(
            snapshot.disk_directories[&tspath::Path::from("/lib")]
                .contains_key(&tspath::Path::from("/lib/utils.ts"))
        );
        assert!(
            snapshot.disk_directories[&tspath::Path::from("/lib")]
                .contains_key(&tspath::Path::from("/lib/helpers.ts"))
        );
        assert!(
            snapshot.disk_directories[&tspath::Path::from("/")]
                .contains_key(&tspath::Path::from("/src"))
        );
        assert!(
            snapshot.disk_directories[&tspath::Path::from("/")]
                .contains_key(&tspath::Path::from("/lib"))
        );
        assert!(
            !snapshot.disk_directories[&tspath::Path::from("/")]
                .contains_key(&tspath::Path::from("/other"))
        );
    }

    // overlay directories are computed from overlays
    {
        let test_fs = vfstest::from_map(HashMap::<&str, &str>::new(), false);
        let overlays = HashMap::from([
            (
                tspath::Path::from("/src/overlay.ts"),
                Arc::new(new_overlay(
                    "/src/overlay.ts".to_string(),
                    "const x = 1;".to_string(),
                    0,
                    ts_core::ScriptKind::TS,
                )),
            ),
            (
                tspath::Path::from("/src/nested/deep.ts"),
                Arc::new(new_overlay(
                    "/src/nested/deep.ts".to_string(),
                    "const y = 2;".to_string(),
                    0,
                    ts_core::ScriptKind::TS,
                )),
            ),
        ]);
        let builder = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            overlays,
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );

        assert!(
            builder.overlay_directories[&tspath::Path::from("/src")]
                .contains_key(&tspath::Path::from("/src/overlay.ts"))
        );
        assert!(
            builder.overlay_directories[&tspath::Path::from("/src")]
                .contains_key(&tspath::Path::from("/src/nested"))
        );
        assert!(
            builder.overlay_directories[&tspath::Path::from("/src/nested")]
                .contains_key(&tspath::Path::from("/src/nested/deep.ts"))
        );
        assert!(
            builder.overlay_directories[&tspath::Path::from("/")]
                .contains_key(&tspath::Path::from("/src"))
        );
    }

    // GetAccessibleEntries combines disk and overlay
    {
        let test_fs =
            vfstest::from_map(HashMap::from([("/src/disk.ts", "const disk = 1;")]), false);
        let overlays = HashMap::from([(
            tspath::Path::from("/src/overlay.ts"),
            Arc::new(new_overlay(
                "/src/overlay.ts".to_string(),
                "const overlay = 1;".to_string(),
                0,
                ts_core::ScriptKind::TS,
            )),
        )]);
        let mut builder = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            overlays,
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );

        let entries = builder.get_accessible_entries("/src");
        assert!(entries.files.contains(&"disk.ts".to_string()));
        assert!(entries.files.contains(&"overlay.ts".to_string()));
    }
}

#[test]
fn test_snapshot_fs() {
    // GetFile returns overlay file
    {
        let test_fs = vfstest::from_map(HashMap::from([("/src/foo.ts", "disk content")]), false);
        let overlays = HashMap::from([(
            tspath::Path::from("/src/foo.ts"),
            Arc::new(new_overlay(
                "/src/foo.ts".to_string(),
                "overlay content".to_string(),
                0,
                ts_core::ScriptKind::TS,
            )),
        )]);
        let snapshot = SnapshotFs {
            to_path: to_path_mapper(),
            fs: Arc::new(test_fs),
            overlays,
            overlay_directories: HashMap::new(),
            disk_files: HashMap::new(),
            disk_directories: HashMap::new(),
            node_modules_realpath_aliases: HashMap::new(),
            read_files: empty_read_files(),
        };

        let fh = snapshot.get_file("/src/foo.ts");
        assert!(fh.is_some());
        assert_eq!(fh.unwrap().content(), "overlay content");
    }

    // GetFile returns disk file when not in overlay
    {
        let test_fs = vfstest::from_map(HashMap::from([("/src/foo.ts", "disk content")]), false);
        let disk_files = HashMap::from([(
            tspath::Path::from("/src/foo.ts"),
            Arc::new(new_disk_file(
                "/src/foo.ts".to_string(),
                "disk content".to_string(),
            )),
        )]);
        let snapshot = SnapshotFs {
            to_path: to_path_mapper(),
            fs: Arc::new(test_fs),
            overlays: HashMap::new(),
            overlay_directories: HashMap::new(),
            disk_files,
            disk_directories: HashMap::new(),
            node_modules_realpath_aliases: HashMap::new(),
            read_files: empty_read_files(),
        };

        let fh = snapshot.get_file("/src/foo.ts");
        assert!(fh.is_some());
        assert_eq!(fh.unwrap().content(), "disk content");
    }

    // GetFile reads from fs when not cached
    {
        let test_fs = vfstest::from_map(HashMap::from([("/src/foo.ts", "fs content")]), false);
        let snapshot = SnapshotFs {
            to_path: to_path_mapper(),
            fs: Arc::new(test_fs),
            overlays: HashMap::new(),
            overlay_directories: HashMap::new(),
            disk_files: HashMap::new(),
            disk_directories: HashMap::new(),
            node_modules_realpath_aliases: HashMap::new(),
            read_files: empty_read_files(),
        };

        let fh = snapshot.get_file("/src/foo.ts");
        assert!(fh.is_some());
        assert_eq!(fh.unwrap().content(), "fs content");
    }

    // GetFile returns nil for non-existent file
    {
        let test_fs = vfstest::from_map(HashMap::<&str, &str>::new(), false);
        let snapshot = SnapshotFs {
            to_path: to_path_mapper(),
            fs: Arc::new(test_fs),
            overlays: HashMap::new(),
            overlay_directories: HashMap::new(),
            disk_files: HashMap::new(),
            disk_directories: HashMap::new(),
            node_modules_realpath_aliases: HashMap::new(),
            read_files: empty_read_files(),
        };

        assert!(snapshot.get_file("/src/nonexistent.ts").is_none());
    }

    // isOpenFile returns true for overlays
    {
        let test_fs = vfstest::from_map(HashMap::<&str, &str>::new(), false);
        let overlays = HashMap::from([(
            tspath::Path::from("/src/foo.ts"),
            Arc::new(new_overlay(
                "/src/foo.ts".to_string(),
                "overlay content".to_string(),
                0,
                ts_core::ScriptKind::TS,
            )),
        )]);
        let snapshot = SnapshotFs {
            to_path: to_path_mapper(),
            fs: Arc::new(test_fs),
            overlays,
            overlay_directories: HashMap::new(),
            disk_files: HashMap::new(),
            disk_directories: HashMap::new(),
            node_modules_realpath_aliases: HashMap::new(),
            read_files: empty_read_files(),
        };

        assert!(snapshot.is_open_file("/src/foo.ts"));
        assert!(!snapshot.is_open_file("/src/bar.ts"));
    }

    // GetFileByPath uses provided path
    {
        let test_fs = vfstest::from_map(HashMap::from([("/src/foo.ts", "disk content")]), false);
        let overlays = HashMap::from([(
            tspath::Path::from("/src/foo.ts"),
            Arc::new(new_overlay(
                "/src/foo.ts".to_string(),
                "overlay content".to_string(),
                0,
                ts_core::ScriptKind::TS,
            )),
        )]);
        let snapshot = SnapshotFs {
            to_path: to_path_mapper(),
            fs: Arc::new(test_fs),
            overlays,
            overlay_directories: HashMap::new(),
            disk_files: HashMap::new(),
            disk_directories: HashMap::new(),
            node_modules_realpath_aliases: HashMap::new(),
            read_files: empty_read_files(),
        };

        let fh = snapshot.get_file_by_path("/src/foo.ts", &tspath::Path::from("/src/foo.ts"));
        assert!(fh.is_some());
        assert_eq!(fh.unwrap().content(), "overlay content");
    }

    // GetAccessibleEntries combines disk and overlay directories
    {
        let test_fs = vfstest::from_map(HashMap::<&str, &str>::new(), false);
        let overlays = HashMap::from([(
            tspath::Path::from("/src/overlay.ts"),
            Arc::new(new_overlay(
                "/src/overlay.ts".to_string(),
                "overlay content".to_string(),
                0,
                ts_core::ScriptKind::TS,
            )),
        )]);
        let overlay_directories = HashMap::from([
            (
                tspath::Path::from("/"),
                HashMap::from([(tspath::Path::from("/src"), "src".to_string())]),
            ),
            (
                tspath::Path::from("/src"),
                HashMap::from([(
                    tspath::Path::from("/src/overlay.ts"),
                    "overlay.ts".to_string(),
                )]),
            ),
        ]);
        let disk_files = HashMap::from([(
            tspath::Path::from("/src/disk.ts"),
            Arc::new(new_disk_file(
                "/src/disk.ts".to_string(),
                "disk content".to_string(),
            )),
        )]);
        let disk_directories = HashMap::from([
            (
                tspath::Path::from("/"),
                HashMap::from([(tspath::Path::from("/src"), "src".to_string())]),
            ),
            (
                tspath::Path::from("/src"),
                HashMap::from([(tspath::Path::from("/src/disk.ts"), "disk.ts".to_string())]),
            ),
        ]);
        let snapshot = SnapshotFs {
            to_path: to_path_mapper(),
            fs: Arc::new(test_fs),
            overlays,
            overlay_directories,
            disk_files,
            disk_directories,
            node_modules_realpath_aliases: HashMap::new(),
            read_files: empty_read_files(),
        };

        let entries = snapshot.get_accessible_entries("/src");
        assert!(entries.files.contains(&"disk.ts".to_string()));
        assert!(entries.files.contains(&"overlay.ts".to_string()));
    }
}

#[test]
fn test_source_fs() {
    // tracks files when tracking enabled
    {
        let test_fs = vfstest::from_map(HashMap::from([("/src/foo.ts", "content")]), false);
        let snapshot = SnapshotFs {
            to_path: to_path_mapper(),
            fs: Arc::new(test_fs),
            overlays: HashMap::new(),
            overlay_directories: HashMap::new(),
            disk_files: HashMap::new(),
            disk_directories: HashMap::new(),
            node_modules_realpath_aliases: HashMap::new(),
            read_files: empty_read_files(),
        };
        let source_fs = new_source_fs(true, snapshot, to_path_mapper());

        assert!(!source_fs.seen_file(&tspath::Path::from("/src/foo.ts")));
        assert!(source_fs.get_file("/src/foo.ts").is_some());
        assert!(source_fs.seen_file(&tspath::Path::from("/src/foo.ts")));
    }

    // does not track files when tracking disabled
    {
        let test_fs = vfstest::from_map(HashMap::from([("/src/foo.ts", "content")]), false);
        let snapshot = SnapshotFs {
            to_path: to_path_mapper(),
            fs: Arc::new(test_fs),
            overlays: HashMap::new(),
            overlay_directories: HashMap::new(),
            disk_files: HashMap::new(),
            disk_directories: HashMap::new(),
            node_modules_realpath_aliases: HashMap::new(),
            read_files: empty_read_files(),
        };
        let source_fs = new_source_fs(false, snapshot, to_path_mapper());

        assert!(source_fs.get_file("/src/foo.ts").is_some());
        assert!(!source_fs.seen_file(&tspath::Path::from("/src/foo.ts")));
    }

    // DisableTracking stops tracking
    {
        let test_fs = vfstest::from_map(
            HashMap::from([("/src/foo.ts", "content"), ("/src/bar.ts", "content")]),
            false,
        );
        let snapshot = SnapshotFs {
            to_path: to_path_mapper(),
            fs: Arc::new(test_fs),
            overlays: HashMap::new(),
            overlay_directories: HashMap::new(),
            disk_files: HashMap::new(),
            disk_directories: HashMap::new(),
            node_modules_realpath_aliases: HashMap::new(),
            read_files: empty_read_files(),
        };
        let source_fs = new_source_fs(true, snapshot, to_path_mapper());

        source_fs.get_file("/src/foo.ts");
        assert!(source_fs.seen_file(&tspath::Path::from("/src/foo.ts")));
        source_fs.disable_tracking();
        source_fs.get_file("/src/bar.ts");
        assert!(!source_fs.seen_file(&tspath::Path::from("/src/bar.ts")));
    }

    // FileExists returns true for files in source
    {
        let test_fs = vfstest::from_map(HashMap::from([("/src/foo.ts", "content")]), false);
        let snapshot = SnapshotFs {
            to_path: to_path_mapper(),
            fs: Arc::new(test_fs),
            overlays: HashMap::new(),
            overlay_directories: HashMap::new(),
            disk_files: HashMap::new(),
            disk_directories: HashMap::new(),
            node_modules_realpath_aliases: HashMap::new(),
            read_files: empty_read_files(),
        };
        let source_fs = new_source_fs(false, snapshot, to_path_mapper());

        assert!(source_fs.file_exists("/src/foo.ts"));
        assert!(!source_fs.file_exists("/src/nonexistent.ts"));
    }

    // ReadFile returns content for files in source
    {
        let test_fs = vfstest::from_map(HashMap::from([("/src/foo.ts", "file content")]), false);
        let snapshot = SnapshotFs {
            to_path: to_path_mapper(),
            fs: Arc::new(test_fs),
            overlays: HashMap::new(),
            overlay_directories: HashMap::new(),
            disk_files: HashMap::new(),
            disk_directories: HashMap::new(),
            node_modules_realpath_aliases: HashMap::new(),
            read_files: empty_read_files(),
        };
        let source_fs = new_source_fs(false, snapshot, to_path_mapper());

        let (content, ok) = source_fs.read_file("/src/foo.ts");
        assert!(ok);
        assert_eq!(content, "file content");
        let (_, ok) = source_fs.read_file("/src/nonexistent.ts");
        assert!(!ok);
    }
}

#[test]
fn test_auto_import_builder_fs() {
    // symlink cache mismatch: file cached at symlink path, missed at realpath after deletion
    {
        let test_fs = vfstest::from_map(
            HashMap::from([
                (
                    "/real/pkg/index.d.ts",
                    map_file("export declare const x: number;"),
                ),
                ("/project/node_modules/pkg", vfstest::symlink("/real/pkg")),
            ]),
            true,
        );
        let symlink_path = "/project/node_modules/pkg/index.d.ts";
        let realpath_path = test_fs.realpath(symlink_path);
        assert_eq!(realpath_path, "/real/pkg/index.d.ts");

        let builder = new_snapshot_fs_builder(
            Arc::new(test_fs.clone()),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );
        let mut auto_import_fs = AutoImportBuilderFs {
            snapshot_fs_builder: builder,
            untracked_files: collections::SyncMap::default(),
        };

        let fh = auto_import_fs.get_file(symlink_path);
        assert!(fh.is_some(), "File should be readable via symlink path");
        assert_eq!(fh.unwrap().content(), "export declare const x: number;");
        test_fs.remove("/real/pkg/index.d.ts").unwrap();

        let fh2 = auto_import_fs.get_file(&realpath_path);
        assert!(
            fh2.is_none(),
            "File should be nil when accessed by realpath after deletion from disk"
        );
    }
}

#[test]
fn test_realpath_alias_lifecycle() {
    // alias recorded when reading symlinked node_modules file
    {
        let test_fs = vfstest::from_map(
            HashMap::from([
                (
                    "/project/node_modules/mylib",
                    vfstest::symlink("/packages/mylib"),
                ),
                (
                    "/packages/mylib/package.json",
                    map_file(r#"{"name": "mylib", "main": "index.js"}"#),
                ),
                (
                    "/packages/mylib/index.d.ts",
                    map_file("export declare const x: number;"),
                ),
                (
                    "/project/node_modules/nolink/package.json",
                    map_file(r#"{"name": "nolink"}"#),
                ),
            ]),
            false,
        );
        let mut builder = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );

        let fh = builder.get_file("/project/node_modules/mylib/package.json");
        assert!(fh.is_some());
        assert_eq!(
            fh.unwrap().content(),
            r#"{"name": "mylib", "main": "index.js"}"#
        );
        assert!(
            builder
                .get_file("/project/node_modules/nolink/package.json")
                .is_some()
        );
        let (snapshot, _) = builder.finalize();

        let aliases = snapshot
            .node_modules_realpath_aliases
            .get(&tspath::Path::from("/packages/mylib/package.json"))
            .expect("alias should exist for realpath of symlinked file");
        assert!(aliases.paths.has(&tspath::Path::from(
            "/project/node_modules/mylib/package.json"
        )));
        assert!(
            !snapshot
                .node_modules_realpath_aliases
                .contains_key(&tspath::Path::from(
                    "/project/node_modules/nolink/package.json"
                )),
            "no alias should exist for non-symlinked file"
        );
    }

    // no alias recorded for files outside node_modules
    {
        let test_fs = vfstest::from_map(
            HashMap::from([
                ("/project/link", vfstest::symlink("/elsewhere")),
                ("/elsewhere/index.ts", map_file("export const x = 1;")),
            ]),
            false,
        );
        let mut builder = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );

        assert!(builder.get_file("/project/link/index.ts").is_some());
        let (snapshot, _) = builder.finalize();
        assert_eq!(snapshot.node_modules_realpath_aliases.len(), 0);
    }

    // aliases carried over across snapshots
    {
        let test_fs = vfstest::from_map(
            HashMap::from([
                (
                    "/project/node_modules/mylib",
                    vfstest::symlink("/packages/mylib"),
                ),
                (
                    "/packages/mylib/package.json",
                    map_file(r#"{"name": "mylib"}"#),
                ),
            ]),
            false,
        );
        let mut builder1 = new_snapshot_fs_builder(
            Arc::new(test_fs.clone()),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );
        builder1.get_file("/project/node_modules/mylib/package.json");
        let (snapshot1, _) = builder1.finalize();

        let builder2 = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            HashMap::new(),
            snapshot1.disk_files.clone(),
            snapshot1.disk_directories.clone(),
            snapshot1.node_modules_realpath_aliases.clone(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );
        let (snapshot2, _) = builder2.finalize();

        let aliases = snapshot2
            .node_modules_realpath_aliases
            .get(&tspath::Path::from("/packages/mylib/package.json"))
            .expect("alias should survive across snapshots");
        assert!(aliases.paths.has(&tspath::Path::from(
            "/project/node_modules/mylib/package.json"
        )));
    }

    // alias pruned when symlinked file is deleted
    {
        let test_fs = vfstest::from_map(
            HashMap::from([
                (
                    "/project/node_modules/mylib",
                    vfstest::symlink("/packages/mylib"),
                ),
                (
                    "/packages/mylib/package.json",
                    map_file(r#"{"name": "mylib"}"#),
                ),
                (
                    "/packages/mylib/index.d.ts",
                    map_file("export declare const x: number;"),
                ),
            ]),
            false,
        );
        let mut builder1 = new_snapshot_fs_builder(
            Arc::new(test_fs.clone()),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );
        builder1.get_file("/project/node_modules/mylib/package.json");
        builder1.get_file("/project/node_modules/mylib/index.d.ts");
        let (snapshot1, _) = builder1.finalize();
        assert!(
            snapshot1
                .node_modules_realpath_aliases
                .contains_key(&tspath::Path::from("/packages/mylib/package.json"))
        );
        assert!(
            snapshot1
                .node_modules_realpath_aliases
                .contains_key(&tspath::Path::from("/packages/mylib/index.d.ts"))
        );

        let builder2 = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            HashMap::new(),
            snapshot1.disk_files.clone(),
            snapshot1.disk_directories.clone(),
            snapshot1.node_modules_realpath_aliases.clone(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );
        if let Some(entry) = builder2.disk_files.load(&tspath::Path::from(
            "/project/node_modules/mylib/index.d.ts",
        )) {
            entry.delete();
        }
        let (snapshot2, _) = builder2.finalize();

        let aliases = snapshot2
            .node_modules_realpath_aliases
            .get(&tspath::Path::from("/packages/mylib/package.json"))
            .expect("package.json alias should survive");
        assert!(aliases.paths.has(&tspath::Path::from(
            "/project/node_modules/mylib/package.json"
        )));
        assert!(
            !snapshot2
                .node_modules_realpath_aliases
                .contains_key(&tspath::Path::from("/packages/mylib/index.d.ts"))
        );
    }

    // multiple symlinks to same realpath
    {
        let test_fs = vfstest::from_map(
            HashMap::from([
                (
                    "/project/node_modules/mylib",
                    vfstest::symlink("/packages/mylib"),
                ),
                (
                    "/project/node_modules/alias",
                    vfstest::symlink("/packages/mylib"),
                ),
                (
                    "/packages/mylib/package.json",
                    map_file(r#"{"name": "mylib"}"#),
                ),
            ]),
            false,
        );
        let mut builder = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );
        assert!(
            builder
                .get_file("/project/node_modules/mylib/package.json")
                .is_some()
        );
        assert!(
            builder
                .get_file("/project/node_modules/alias/package.json")
                .is_some()
        );
        let (snapshot, _) = builder.finalize();

        let aliases = snapshot
            .node_modules_realpath_aliases
            .get(&tspath::Path::from("/packages/mylib/package.json"))
            .unwrap();
        assert!(aliases.paths.has(&tspath::Path::from(
            "/project/node_modules/mylib/package.json"
        )));
        assert!(aliases.paths.has(&tspath::Path::from(
            "/project/node_modules/alias/package.json"
        )));
    }

    // multiple symlinks pruned individually
    {
        let test_fs = vfstest::from_map(
            HashMap::from([
                (
                    "/project/node_modules/mylib",
                    vfstest::symlink("/packages/mylib"),
                ),
                (
                    "/project/node_modules/alias",
                    vfstest::symlink("/packages/mylib"),
                ),
                (
                    "/packages/mylib/package.json",
                    map_file(r#"{"name": "mylib"}"#),
                ),
            ]),
            false,
        );
        let mut builder1 = new_snapshot_fs_builder(
            Arc::new(test_fs.clone()),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );
        builder1.get_file("/project/node_modules/mylib/package.json");
        builder1.get_file("/project/node_modules/alias/package.json");
        let (snapshot1, _) = builder1.finalize();

        let builder2 = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            HashMap::new(),
            snapshot1.disk_files.clone(),
            snapshot1.disk_directories.clone(),
            snapshot1.node_modules_realpath_aliases.clone(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );
        if let Some(entry) = builder2.disk_files.load(&tspath::Path::from(
            "/project/node_modules/alias/package.json",
        )) {
            entry.delete();
        }
        let (snapshot2, _) = builder2.finalize();

        let aliases = snapshot2
            .node_modules_realpath_aliases
            .get(&tspath::Path::from("/packages/mylib/package.json"))
            .unwrap();
        assert!(aliases.paths.has(&tspath::Path::from(
            "/project/node_modules/mylib/package.json"
        )));
        assert!(!aliases.paths.has(&tspath::Path::from(
            "/project/node_modules/alias/package.json"
        )));
    }

    // expandRealpathAliases expands change events
    {
        let snapshot = snapshot_with_mylib_alias();
        let mut change = FileChangeSummary::default();
        change
            .changed
            .insert("file:///packages/mylib/package.json".to_string());

        let expanded = snapshot.expand_realpath_aliases(change);
        assert!(
            expanded
                .changed
                .contains("file:///packages/mylib/package.json")
        );
        assert!(
            expanded
                .changed
                .contains("file:///project/node_modules/mylib/package.json")
        );
    }

    // expandRealpathAliases expands delete events
    {
        let snapshot = snapshot_with_mylib_alias();
        let mut change = FileChangeSummary::default();
        change
            .deleted
            .insert("file:///packages/mylib/package.json".to_string());

        let expanded = snapshot.expand_realpath_aliases(change);
        assert!(
            expanded
                .deleted
                .contains("file:///project/node_modules/mylib/package.json")
        );
    }

    // expandRealpathAliases is a no-op with no aliases
    {
        let snapshot = SnapshotFs {
            to_path: to_path_mapper(),
            fs: Arc::new(vfstest::from_map(HashMap::<&str, &str>::new(), false)),
            overlays: HashMap::new(),
            overlay_directories: HashMap::new(),
            disk_files: HashMap::new(),
            disk_directories: HashMap::new(),
            node_modules_realpath_aliases: HashMap::new(),
            read_files: empty_read_files(),
        };
        let mut change = FileChangeSummary::default();
        change.changed.insert("file:///some/file.ts".to_string());

        let expanded = snapshot.expand_realpath_aliases(change);
        assert_eq!(expanded.changed.len(), 1);
        assert!(expanded.changed.contains("file:///some/file.ts"));
    }

    // markDirtyFiles invalidates symlinked file via realpath event
    {
        let test_fs = vfstest::from_map(
            HashMap::from([
                (
                    "/project/node_modules/mylib",
                    vfstest::symlink("/packages/mylib"),
                ),
                (
                    "/packages/mylib/package.json",
                    map_file(r#"{"name": "mylib", "main": "index.js"}"#),
                ),
            ]),
            false,
        );
        let mut builder1 = new_snapshot_fs_builder(
            Arc::new(test_fs.clone()),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );
        let fh = builder1.get_file("/project/node_modules/mylib/package.json");
        assert!(fh.is_some());
        assert_eq!(
            fh.unwrap().content(),
            r#"{"name": "mylib", "main": "index.js"}"#
        );
        let (snapshot1, _) = builder1.finalize();

        test_fs
            .write_file("/packages/mylib/package.json", r#"{"name": "mylib"}"#)
            .unwrap();
        let mut builder2 = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            HashMap::new(),
            snapshot1.disk_files.clone(),
            snapshot1.disk_directories.clone(),
            snapshot1.node_modules_realpath_aliases.clone(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );

        let mut change = FileChangeSummary::default();
        change
            .changed
            .insert("file:///packages/mylib/package.json".to_string());
        change = snapshot1.expand_realpath_aliases(change);
        builder2.mark_dirty_files(&change);

        let fh = builder2.get_file("/project/node_modules/mylib/package.json");
        assert!(fh.is_some());
        assert_eq!(fh.unwrap().content(), r#"{"name": "mylib"}"#);
        let (snapshot2, _) = builder2.finalize();
        let file = snapshot2
            .disk_files
            .get(&tspath::Path::from(
                "/project/node_modules/mylib/package.json",
            ))
            .unwrap();
        assert_eq!(file.content(), r#"{"name": "mylib"}"#);
    }

    // alias clone isolation between snapshots
    {
        let (snapshot1, snapshot2) = mylib_then_other_snapshots();
        assert!(
            snapshot1
                .node_modules_realpath_aliases
                .contains_key(&tspath::Path::from("/packages/mylib/package.json"))
        );
        assert!(
            !snapshot1
                .node_modules_realpath_aliases
                .contains_key(&tspath::Path::from("/packages/other/package.json"))
        );
        assert!(
            snapshot2
                .node_modules_realpath_aliases
                .contains_key(&tspath::Path::from("/packages/mylib/package.json"))
        );
        assert!(
            snapshot2
                .node_modules_realpath_aliases
                .contains_key(&tspath::Path::from("/packages/other/package.json"))
        );
    }

    // adding symlink to inherited realpath key does not mutate previous snapshot
    {
        let test_fs = vfstest::from_map(
            HashMap::from([
                (
                    "/project/node_modules/mylib",
                    vfstest::symlink("/packages/mylib"),
                ),
                (
                    "/project/node_modules/alias",
                    vfstest::symlink("/packages/mylib"),
                ),
                (
                    "/packages/mylib/package.json",
                    map_file(r#"{"name": "mylib"}"#),
                ),
            ]),
            false,
        );
        let mut builder1 = new_snapshot_fs_builder(
            Arc::new(test_fs.clone()),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );
        builder1.get_file("/project/node_modules/mylib/package.json");
        let (snapshot1, _) = builder1.finalize();
        let aliases1 = snapshot1
            .node_modules_realpath_aliases
            .get(&tspath::Path::from("/packages/mylib/package.json"))
            .unwrap();
        assert_eq!(aliases1.paths.len(), 1);
        assert!(aliases1.paths.has(&tspath::Path::from(
            "/project/node_modules/mylib/package.json"
        )));

        let mut builder2 = new_snapshot_fs_builder(
            Arc::new(test_fs),
            HashMap::new(),
            HashMap::new(),
            snapshot1.disk_files.clone(),
            snapshot1.disk_directories.clone(),
            snapshot1.node_modules_realpath_aliases.clone(),
            lsproto::PositionEncodingKind::UTF16,
            to_path_mapper(),
        );
        builder2.get_file("/project/node_modules/alias/package.json");
        let (snapshot2, _) = builder2.finalize();

        let aliases2 = snapshot2
            .node_modules_realpath_aliases
            .get(&tspath::Path::from("/packages/mylib/package.json"))
            .unwrap();
        assert_eq!(aliases2.paths.len(), 2);
        assert!(aliases2.paths.has(&tspath::Path::from(
            "/project/node_modules/mylib/package.json"
        )));
        assert!(aliases2.paths.has(&tspath::Path::from(
            "/project/node_modules/alias/package.json"
        )));
        assert_eq!(aliases1.paths.len(), 1);
        assert!(!aliases1.paths.has(&tspath::Path::from(
            "/project/node_modules/alias/package.json"
        )));
    }
}

fn snapshot_with_mylib_alias() -> SnapshotFs {
    let test_fs = vfstest::from_map(
        HashMap::from([
            (
                "/project/node_modules/mylib",
                vfstest::symlink("/packages/mylib"),
            ),
            (
                "/packages/mylib/package.json",
                map_file(r#"{"name": "mylib"}"#),
            ),
        ]),
        false,
    );
    let mut builder = new_snapshot_fs_builder(
        Arc::new(test_fs),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        lsproto::PositionEncodingKind::UTF16,
        to_path_mapper(),
    );
    builder.get_file("/project/node_modules/mylib/package.json");
    builder.finalize().0
}

fn mylib_then_other_snapshots() -> (SnapshotFs, SnapshotFs) {
    let test_fs = vfstest::from_map(
        HashMap::from([
            (
                "/project/node_modules/mylib",
                vfstest::symlink("/packages/mylib"),
            ),
            (
                "/project/node_modules/other",
                vfstest::symlink("/packages/other"),
            ),
            (
                "/packages/mylib/package.json",
                map_file(r#"{"name": "mylib"}"#),
            ),
            (
                "/packages/other/package.json",
                map_file(r#"{"name": "other"}"#),
            ),
        ]),
        false,
    );
    let mut builder1 = new_snapshot_fs_builder(
        Arc::new(test_fs.clone()),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        lsproto::PositionEncodingKind::UTF16,
        to_path_mapper(),
    );
    builder1.get_file("/project/node_modules/mylib/package.json");
    let (snapshot1, _) = builder1.finalize();

    let mut builder2 = new_snapshot_fs_builder(
        Arc::new(test_fs),
        HashMap::new(),
        HashMap::new(),
        snapshot1.disk_files.clone(),
        snapshot1.disk_directories.clone(),
        snapshot1.node_modules_realpath_aliases.clone(),
        lsproto::PositionEncodingKind::UTF16,
        to_path_mapper(),
    );
    builder2.get_file("/project/node_modules/other/package.json");
    let (snapshot2, _) = builder2.finalize();
    (snapshot1, snapshot2)
}
