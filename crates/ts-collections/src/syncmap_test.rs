use crate::SyncMap;

#[test]
fn sync_map_with_nil() {
    let m = SyncMap::<String, ()>::default();

    let (got1, ok) = m.load(&"foo".to_string());
    assert!(!ok);
    assert_eq!(got1, None);

    m.store("foo".to_string(), None);

    let (got2, ok) = m.load(&"foo".to_string());
    assert!(ok);
    assert_eq!(got2, None);

    let (too, loaded) = m.load_or_store("too".to_string(), None);
    assert!(!loaded);
    assert_eq!(too, None);

    m.range(|_, _| true);
}
