use std::collections::BTreeMap;

use crate::osvfs;
use crate::vfs::Fs;
use crate::vfstest::from_map;

#[test]
fn benchmark_read_file_scenarios_are_represented() {
    let small_data = "hello, world";
    let map_fs = from_map(
        BTreeMap::from([("/foo.ts".to_owned(), small_data.to_owned())]),
        true,
    );
    assert_eq!(map_fs.read_file("/foo.ts"), (small_data.to_owned(), true));

    let os_fs = osvfs::os::fs();
    let os_small_data_path =
        std::env::temp_dir().join(format!("tsgo-vfs-readfile-{}-foo.ts", std::process::id()));
    let path = os_small_data_path.to_string_lossy().into_owned();
    os_fs.write_file(&path, small_data).unwrap();
    assert_eq!(os_fs.read_file(&path), (small_data.to_owned(), true));
    let _ = std::fs::remove_file(os_small_data_path);
}
