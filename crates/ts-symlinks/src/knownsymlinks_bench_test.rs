use super::*;
use ts_tspath as tspath;

#[test]
fn benchmark_populate_symlinks_from_resolutions_smoke() {
    let mut cache = new_known_symlink("/project", true);
    let deps = (0..50)
        .map(|i| {
            let suffix = char::from(b'A' + i as u8);
            (
                format!("/project/node_modules/pkg{suffix}/index.js"),
                format!("/real/pkg{suffix}/index.js"),
            )
        })
        .collect::<Vec<_>>();

    for (orig, resolved) in deps {
        cache.process_resolution(orig, resolved);
    }

    assert_eq!(cache.files().len(), 50);
}

#[test]
fn benchmark_set_file_smoke() {
    let mut cache = new_known_symlink("/project", true);
    let symlink = "/project/file.ts";
    let path = tspath::to_path(symlink, "/project", true);

    cache.set_file(
        symlink.to_string(),
        path.clone(),
        "/real/file.ts".to_string(),
    );
    assert_eq!(cache.files().get(&path), Some(&"/real/file.ts".to_string()));
}

#[test]
fn benchmark_set_directory_smoke() {
    let mut cache = new_known_symlink("/project", true);
    let symlink_path = tspath::ensure_trailing_directory_separator(&tspath::to_path(
        "/project/symlink",
        "/project",
        true,
    ));
    let real_dir = KnownDirectoryLink {
        real: "/real/path/".to_string(),
        real_path: tspath::ensure_trailing_directory_separator(&tspath::to_path(
            "/real/path",
            "/project",
            true,
        )),
    };

    cache.set_directory(
        "/project/symlink".to_string(),
        symlink_path.clone(),
        Some(real_dir),
    );
    assert!(cache.directories().contains_key(&symlink_path));
}

#[test]
fn benchmark_guess_directory_symlink_smoke() {
    let cache = new_known_symlink("/project", true);

    let actual = cache.guess_directory_symlink(
        "/real/node_modules/package/dist/index.js",
        "/project/symlink/package/dist/index.js",
        "/project",
    );
    assert_eq!(
        actual,
        (
            "/real/node_modules/package".to_string(),
            "/project/symlink/package".to_string()
        )
    );
}

#[test]
fn benchmark_concurrent_access_smoke() {
    let mut cache = new_known_symlink("/project", true);

    for i in 0..26 {
        let symlink = format!("/project/file{}.ts", char::from(b'A' + i));
        let path = tspath::to_path(&symlink, "/project", true);
        cache.set_file(symlink, path.clone(), "/real/file.ts".to_string());
        let _ = cache.files().get(&path);
    }

    assert_eq!(cache.files().len(), 26);
}
