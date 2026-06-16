use lsp_types_full as lsproto;

use crate::watch::get_path_components_for_watching;
use crate::{WatchedFiles, WatcherId, new_watch_registry};

#[test]
fn test_get_path_components_for_watching() {
    assert_eq!(
        get_path_components_for_watching("/project", ""),
        vec!["/", "project"]
    );
    assert_eq!(
        get_path_components_for_watching("C:\\project", ""),
        vec!["C:/", "project"]
    );
    assert_eq!(
        get_path_components_for_watching("//server/share/project/tsconfig.json", ""),
        vec!["//server/share", "project", "tsconfig.json"]
    );
    assert_eq!(
        get_path_components_for_watching(r"\\server\share\project\tsconfig.json", ""),
        vec!["//server/share", "project", "tsconfig.json"]
    );
    assert_eq!(
        get_path_components_for_watching("C:\\Users", ""),
        vec!["C:/Users"]
    );
    assert_eq!(
        get_path_components_for_watching("C:\\Users\\andrew\\project", ""),
        vec!["C:/Users/andrew", "project"]
    );
    assert_eq!(get_path_components_for_watching("/home", ""), vec!["/home"]);
    assert_eq!(
        get_path_components_for_watching("/home/andrew/project", ""),
        vec!["/home/andrew", "project"]
    );
}

#[test]
fn test_nil_watched_files_clone() {
    let watched_files = None::<WatchedFiles<i32>>;
    let result = watched_files.map(|watch| watch.clone_with_input(42));

    assert!(
        result.is_none(),
        "clone on a nil `WatchedFiles` should return nil"
    );
}

#[test]
fn test_watch_registry_tracks_ref_counts_and_pending_watchers() {
    let registry = new_watch_registry();
    let watcher = lsproto::FileSystemWatcher {
        glob_pattern: lsproto::GlobPattern::String("/workspace/**/*.ts".to_owned()),
        kind: Some(lsproto::WatchKind::Create | lsproto::WatchKind::Change),
    };
    let id = WatcherId("wildcardDirectories watcher 1".to_owned());

    assert!(registry.acquire(&watcher, id.clone()));
    assert!(!registry.acquire(&watcher, WatcherId("unused replacement".to_owned())));

    registry.mark_pending(id.clone());
    assert!(registry.is_pending(&id));
    registry.clear_pending(&id);
    assert!(!registry.is_pending(&id));

    assert_eq!(registry.release(&watcher), (WatcherId::default(), false));
    assert_eq!(registry.release(&watcher), (id, true));
}

#[test]
fn test_watcher_id_preserves_string_identity() {
    let id = WatcherId::from("programFiles watcher 2");

    assert_eq!(id.as_str(), "programFiles watcher 2");
    assert_eq!(id.to_string(), "programFiles watcher 2");
    assert_eq!(WatcherId::from(id.to_string()), id);
}
