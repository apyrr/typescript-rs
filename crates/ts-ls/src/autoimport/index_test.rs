use super::index::{Index, Named};

#[derive(Clone, Debug, Eq, PartialEq)]
struct TestEntry {
    name: String,
    package_: String,
}

impl Named for TestEntry {
    fn name(&self) -> &str {
        &self.name
    }
}

#[test]
fn test_index_clone_filters_entries_by_package() {
    let mut idx = Index::<TestEntry>::default();
    idx.insert_as_words(TestEntry {
        name: "fooBar".to_string(),
        package_: "pkg-a".to_string(),
    });
    idx.insert_as_words(TestEntry {
        name: "bazQux".to_string(),
        package_: "pkg-b".to_string(),
    });
    idx.insert_as_words(TestEntry {
        name: "fooQux".to_string(),
        package_: "pkg-a".to_string(),
    });

    // Clone excluding pkg-b
    let cloned = idx
        .clone_filtered(|e| e.package_ != "pkg-b")
        .expect("clone_filtered should always return an index for a live Rust value");

    // Original should have all 3 entries
    assert_eq!(idx.entries.len(), 3);

    // Cloned should have 2 entries (only pkg-a)
    assert_eq!(cloned.entries.len(), 2);

    // Search should work on cloned index
    let results = cloned.find("fooBar", true);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "fooBar");

    // bazQux should not be in cloned index
    let results = cloned.find("bazQux", true);
    assert_eq!(results.len(), 0);

    // Word prefix search should work
    let results = cloned.search_word_prefix("foo");
    assert_eq!(results.len(), 2);
}

#[test]
fn test_index_clone_handles_empty_index() {
    let idx = Index::<TestEntry>::default();
    let cloned = idx
        .clone_filtered(|_| true)
        .expect("clone_filtered should return an empty index for an empty input");
    assert_eq!(cloned.entries.len(), 0);
}

#[test]
fn test_index_clone_filters_all_entries() {
    let mut idx = Index::<TestEntry>::default();
    idx.insert_as_words(TestEntry {
        name: "fooBar".to_string(),
        package_: "pkg-a".to_string(),
    });
    idx.insert_as_words(TestEntry {
        name: "bazQux".to_string(),
        package_: "pkg-b".to_string(),
    });

    let cloned = idx
        .clone_filtered(|_| false)
        .expect("clone_filtered should return an empty filtered index");
    assert_eq!(cloned.entries.len(), 0);
    assert_eq!(cloned.index.len(), 0);
}
