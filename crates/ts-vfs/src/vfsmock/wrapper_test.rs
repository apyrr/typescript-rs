use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use crate::vfs::Fs;
use crate::vfstest::from_map;

use super::wrapper::wrap;

#[test]
fn test_wrap() {
    let fs = from_map(
        BTreeMap::from([("/foo.ts".to_owned(), "hello".to_owned())]),
        true,
    );
    let wrapper = wrap(fs);

    assert!(wrapper.use_case_sensitive_file_names());
    assert!(wrapper.file_exists("/foo.ts"));
    assert!(!wrapper.directory_exists("/foo.ts"));
    assert_eq!(wrapper.read_file("/foo.ts"), ("hello".to_owned(), true));
    assert_eq!(wrapper.realpath("/foo.ts"), "/foo.ts");
    assert_eq!(
        wrapper.get_accessible_entries("/").files,
        vec!["foo.ts".to_owned()]
    );
    assert!(wrapper.stat("/foo.ts").unwrap().is_file());

    wrapper.append_file("/foo.ts", ", world").unwrap();
    assert_eq!(
        wrapper.read_file("/foo.ts"),
        ("hello, world".to_owned(), true)
    );
    let mtime = SystemTime::UNIX_EPOCH + Duration::from_secs(5);
    wrapper.chtimes("/foo.ts", mtime, mtime).unwrap();
    assert_eq!(wrapper.stat("/foo.ts").unwrap().modified().unwrap(), mtime);
    wrapper.write_file("/bar.ts", "bar").unwrap();
    assert!(wrapper.file_exists("/bar.ts"));
    wrapper.remove("/bar.ts").unwrap();
    assert!(!wrapper.file_exists("/bar.ts"));

    let mut walked = Vec::new();
    wrapper
        .walk_dir("/", &mut |path, _entry, err| {
            assert!(err.is_none());
            walked.push(path.to_owned());
            Ok(())
        })
        .unwrap();
    assert!(walked.iter().any(|path| path == "/foo.ts"));
}
