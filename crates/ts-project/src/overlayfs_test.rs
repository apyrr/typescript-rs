use std::{collections::HashMap, sync::Arc};

use ts_lsproto as lsproto;
use ts_tspath as tspath;
use ts_vfs::vfstest;

use crate::{FileChange, FileChangeKind, Overlay, OverlayFs, new_overlay_fs};

const LANGUAGE_KIND_TYPESCRIPT: &str = "typescript";

fn create_overlay_fs() -> OverlayFs {
    let test_fs = vfstest::from_map(
        HashMap::from([
            ("/test1.ts".to_string(), "// existing content".to_string()),
            ("/test2.ts".to_string(), "// existing content".to_string()),
        ]),
        false,
    );
    new_overlay_fs(
        Arc::new(test_fs),
        HashMap::<tspath::Path, Arc<Overlay>>::new(),
        lsproto::PositionEncodingKind::UTF16,
        |file_name| file_name.into(),
    )
}

#[test]
fn test_process_changes() {
    let test_uri1 = "file:///test1.ts".to_string();
    let test_uri2 = "file:///test2.ts".to_string();

    // "multiple opens should panic"
    {
        let fs = create_overlay_fs();
        let changes = vec![
            FileChange {
                kind: FileChangeKind::Open,
                uri: test_uri1.clone(),
                version: 1,
                content: "const x = 1;".to_string(),
                language_kind: LANGUAGE_KIND_TYPESCRIPT.to_string(),
                changes: Vec::new(),
            },
            FileChange {
                kind: FileChangeKind::Open,
                uri: test_uri2.clone(),
                version: 1,
                content: "const y = 2;".to_string(),
                language_kind: LANGUAGE_KIND_TYPESCRIPT.to_string(),
                changes: Vec::new(),
            },
        ];

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = fs.process_changes(changes);
            }))
            .is_err()
        );
    }

    // "watch create then delete becomes nothing"
    {
        let fs = create_overlay_fs();
        let changes = vec![
            FileChange {
                kind: FileChangeKind::WatchCreate,
                uri: test_uri1.clone(),
                version: 0,
                content: String::new(),
                language_kind: String::new(),
                changes: Vec::new(),
            },
            FileChange {
                kind: FileChangeKind::WatchDelete,
                uri: test_uri1.clone(),
                version: 0,
                content: String::new(),
                language_kind: String::new(),
                changes: Vec::new(),
            },
        ];

        let (result, _) = fs.process_changes(changes);
        assert!(result.is_empty());
    }

    // "watch delete then create becomes change"
    {
        let fs = create_overlay_fs();
        let changes = vec![
            FileChange {
                kind: FileChangeKind::WatchDelete,
                uri: test_uri1.clone(),
                version: 0,
                content: String::new(),
                language_kind: String::new(),
                changes: Vec::new(),
            },
            FileChange {
                kind: FileChangeKind::WatchCreate,
                uri: test_uri1.clone(),
                version: 0,
                content: String::new(),
                language_kind: String::new(),
                changes: Vec::new(),
            },
        ];

        let (result, _) = fs.process_changes(changes);
        assert!(result.created.is_empty());
        assert!(result.deleted.is_empty());
        assert!(result.changed.contains(&test_uri1));
    }

    // "multiple watch changes deduplicated"
    {
        let fs = create_overlay_fs();
        let changes = vec![
            FileChange {
                kind: FileChangeKind::WatchChange,
                uri: test_uri1.clone(),
                version: 0,
                content: String::new(),
                language_kind: String::new(),
                changes: Vec::new(),
            },
            FileChange {
                kind: FileChangeKind::WatchChange,
                uri: test_uri1.clone(),
                version: 0,
                content: String::new(),
                language_kind: String::new(),
                changes: Vec::new(),
            },
            FileChange {
                kind: FileChangeKind::WatchChange,
                uri: test_uri1.clone(),
                version: 0,
                content: String::new(),
                language_kind: String::new(),
                changes: Vec::new(),
            },
        ];

        let (result, _) = fs.process_changes(changes);
        assert!(result.changed.contains(&test_uri1));
        assert_eq!(1, result.changed.len());
    }

    // "save marks overlay as matching disk"
    {
        let fs = create_overlay_fs();

        // First create an overlay
        let _ = fs.process_changes(vec![FileChange {
            kind: FileChangeKind::Open,
            uri: test_uri1.clone(),
            version: 1,
            content: "const x = 1;".to_string(),
            language_kind: LANGUAGE_KIND_TYPESCRIPT.to_string(),
            changes: Vec::new(),
        }]);
        // Then save
        let (result, _) = fs.process_changes(vec![FileChange {
            kind: FileChangeKind::Save,
            uri: test_uri1.clone(),
            version: 0,
            content: String::new(),
            language_kind: String::new(),
            changes: Vec::new(),
        }]);
        // We don't observe saves for snapshot changes,
        // so they're not included in the summary
        assert!(result.is_empty());

        // Check that the overlay is marked as matching disk text
        let fh = fs.get_file("/test1.ts".to_string());
        assert!(fh.is_some());
        assert!(fh.unwrap().matches_disk_text());
    }

    // "watch change on overlay marks as not matching disk"
    {
        let fs = create_overlay_fs();

        // First create an overlay
        let _ = fs.process_changes(vec![FileChange {
            kind: FileChangeKind::Open,
            uri: test_uri1.clone(),
            version: 1,
            content: "const x = 1;".to_string(),
            language_kind: LANGUAGE_KIND_TYPESCRIPT.to_string(),
            changes: Vec::new(),
        }]);
        assert!(
            !fs.get_file("/test1.ts".to_string())
                .unwrap()
                .matches_disk_text()
        );

        // Then save
        let _ = fs.process_changes(vec![FileChange {
            kind: FileChangeKind::Save,
            uri: test_uri1.clone(),
            version: 0,
            content: String::new(),
            language_kind: String::new(),
            changes: Vec::new(),
        }]);
        assert!(
            fs.get_file("/test1.ts".to_string())
                .unwrap()
                .matches_disk_text()
        );

        // Now process a watch change
        let _ = fs.process_changes(vec![FileChange {
            kind: FileChangeKind::WatchChange,
            uri: test_uri1.clone(),
            version: 0,
            content: String::new(),
            language_kind: String::new(),
            changes: Vec::new(),
        }]);
        assert!(
            !fs.get_file("/test1.ts".to_string())
                .unwrap()
                .matches_disk_text()
        );
    }

    // "close then open in same batch marks as changed"
    {
        let fs = create_overlay_fs();

        // First create an overlay
        let _ = fs.process_changes(vec![FileChange {
            kind: FileChangeKind::Open,
            uri: test_uri1.clone(),
            version: 1,
            content: "const x = 1;".to_string(),
            language_kind: LANGUAGE_KIND_TYPESCRIPT.to_string(),
            changes: Vec::new(),
        }]);

        // Now close and reopen in the same batch (like Neovim does for file reload)
        let (result, _) = fs.process_changes(vec![
            FileChange {
                kind: FileChangeKind::Close,
                uri: test_uri1.clone(),
                version: 0,
                content: String::new(),
                language_kind: String::new(),
                changes: Vec::new(),
            },
            FileChange {
                kind: FileChangeKind::Open,
                uri: test_uri1.clone(),
                version: 0,
                content: "const x = 2;".to_string(),
                language_kind: LANGUAGE_KIND_TYPESCRIPT.to_string(),
                changes: Vec::new(),
            },
        ]);

        // Should not be marked as opened since it was already open
        assert!(
            result.opened.is_empty(),
            "close then open should not mark as opened"
        );
        // Should also be marked as changed since it was closed and reopened
        assert!(
            result.changed.contains(&test_uri1),
            "close then open should mark as changed"
        );
        // Should have the new content
        let fh = fs.get_file("/test1.ts".to_string()).unwrap();
        assert_eq!("const x = 2;", fh.content());
    }
}
