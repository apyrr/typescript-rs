#[cfg(windows)]
use std::{fs, process::Command};

#[cfg(windows)]
use super::reparsepoint_windows::is_reparse_point;

#[cfg(windows)]
fn mklink(target: &std::path::Path, link: &std::path::Path, junction: bool) {
    let mut command = Command::new("cmd");
    command.arg("/c").arg("mklink");
    if junction {
        command.arg("/J");
    }
    command.arg(link).arg(target);
    assert!(command.status().unwrap().success());
}

#[cfg(windows)]
#[test]
fn test_is_reparse_point() {
    let tmp = std::env::temp_dir().join(format!("tsgo-reparse-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let file = tmp.join("regular.txt");
    fs::write(&file, "hello").unwrap();
    assert!(!is_reparse_point(&file.to_string_lossy()));

    let dir = tmp.join("regular-dir");
    fs::create_dir_all(&dir).unwrap();
    assert!(!is_reparse_point(&dir.to_string_lossy()));

    let target = tmp.join("junction-target");
    let link = tmp.join("junction-link");
    fs::create_dir_all(&target).unwrap();
    mklink(&target, &link, true);
    assert!(is_reparse_point(&link.to_string_lossy()));

    let target_file = tmp.join("symlink-target.txt");
    let link_file = tmp.join("symlink-link.txt");
    fs::write(&target_file, "hello").unwrap();
    mklink(&target_file, &link_file, false);
    assert!(is_reparse_point(&link_file.to_string_lossy()));

    assert!(!is_reparse_point(
        &tmp.join("does-not-exist").to_string_lossy()
    ));
    assert!(!is_reparse_point(""));
    assert!(!is_reparse_point("invalid\0path"));

    let _ = fs::remove_dir_all(&tmp);
}

#[cfg(windows)]
#[test]
fn test_is_reparse_point_long_path() {
    let tmp = std::env::temp_dir().join(format!("tsgo-reparse-long-{}", std::process::id()));
    let mut long_path_base = tmp;
    let path_component =
        "very_long_directory_name_to_exceed_max_path_limit_abcdefghijklmnopqrstuvwxyz";
    while long_path_base.to_string_lossy().len() < 250 {
        long_path_base = long_path_base.join(path_component);
    }
    let target = long_path_base.join("target");
    let link = long_path_base.join("link");
    fs::create_dir_all(format!(r"\\?\{}", target.to_string_lossy())).unwrap();
    mklink(
        std::path::Path::new(&format!(r"\\?\{}", target.to_string_lossy())),
        std::path::Path::new(&format!(r"\\?\{}", link.to_string_lossy())),
        true,
    );
    assert!(is_reparse_point(&link.to_string_lossy()));
}

#[cfg(windows)]
#[test]
fn test_is_reparse_point_nested_in_symlink() {
    let tmp = std::env::temp_dir().join(format!("tsgo-reparse-nested-{}", std::process::id()));
    let target = tmp.join("target");
    let inner_target = target.join("inner-target");
    fs::create_dir_all(&inner_target).unwrap();
    let link = tmp.join("link");
    mklink(&target, &link, true);
    let inner_link = target.join("inner-link");
    mklink(&inner_target, &inner_link, true);
    assert!(is_reparse_point(&link.join("inner-link").to_string_lossy()));
}

#[cfg(windows)]
#[test]
fn benchmark_is_symlink_or_junction_scenarios_are_represented() {
    let tmp = std::env::temp_dir().join(format!("tsgo-reparse-bench-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let regular_file = tmp.join("regular.txt");
    fs::write(&regular_file, "hello").unwrap();
    let target = tmp.join("target");
    let link = tmp.join("link");
    fs::create_dir_all(&target).unwrap();
    mklink(&target, &link, true);
    assert!(!is_reparse_point(&regular_file.to_string_lossy()));
    assert!(is_reparse_point(&link.to_string_lossy()));
    assert!(!is_reparse_point(
        &tmp.join("does-not-exist").to_string_lossy()
    ));
}

#[cfg(not(windows))]
#[test]
fn reparse_point_tests_are_windows_only() {
    // PORT NOTE: Go source is Windows-specific; non-Windows Rust test target is intentionally inert.
}
