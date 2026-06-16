use lsp_types_full as lsproto;

use crate::{WatcherId, new_watch_registry};

#[test]
fn test_update_watch_timeout_and_rollback() {
    let registry = new_watch_registry();
    let watcher = lsproto::FileSystemWatcher {
        glob_pattern: lsproto::GlobPattern::String("/home/projects/TS/p1/src/**/*.ts".to_owned()),
        kind: Some(
            lsproto::WatchKind::Create | lsproto::WatchKind::Change | lsproto::WatchKind::Delete,
        ),
    };
    let batch_id = WatcherId("programFiles".to_owned());
    let glob_id = WatcherId("programFiles.0".to_owned());

    assert!(registry.acquire(&watcher, glob_id.clone()));

    let (removed_id, removed) = registry.release(&watcher);
    assert!(removed);
    assert_eq!(removed_id, glob_id);
    registry.mark_pending(batch_id.clone());
    assert!(registry.is_pending(&batch_id));

    let retry_glob_id = WatcherId("programFiles.0".to_owned());
    assert!(registry.acquire(&watcher, retry_glob_id.clone()));
    registry.clear_pending(&batch_id);
    assert!(!registry.is_pending(&batch_id));

    let (removed_retry_id, removed_retry) = registry.release(&watcher);
    assert!(removed_retry);
    assert_eq!(removed_retry_id, retry_glob_id);
}
