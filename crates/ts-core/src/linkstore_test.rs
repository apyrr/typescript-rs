use super::LinkStore;
#[cfg(feature = "link_store_stats")]
use super::{link_store_stats_snapshot, reset_link_store_stats, set_link_store_stats_enabled};

#[derive(Default, Debug, Eq, PartialEq)]
struct Links {
    value: usize,
}

#[test]
fn try_get_does_not_create_entry() {
    let links = LinkStore::<&'static str, Links>::default();

    assert!(links.try_handle("missing").is_none());
    assert!(!links.has("missing"));
    assert!(links.is_empty());
}

#[test]
fn get_creates_default_entry_and_preserves_mutation() {
    let links = LinkStore::<&'static str, Links>::default();

    let handle = links.ensure_handle("key");
    links.with_by_handle_mut(handle, |links| links.value = 42);

    assert!(links.has("key"));
    assert_eq!(links.with_by_handle(handle, |links| links.value), 42);
}

#[test]
fn unkeyed_handles_store_values_without_key_entries() {
    let links = LinkStore::<&'static str, Links>::default();

    let handle = links.allocate_unkeyed_handle();
    links.with_by_handle_mut(handle, |links| links.value = 42);

    assert!(links.is_empty());
    assert!(links.try_handle("key").is_none());
    assert_eq!(links.with_by_handle(handle, |links| links.value), 42);
}

#[test]
fn extend_from_moves_entries_and_overwrites_duplicate_keys() {
    let mut target = LinkStore::<&'static str, Links>::default();
    let keep = target.ensure_handle("keep");
    let replace = target.ensure_handle("replace");
    target.with_by_handle_mut(keep, |links| links.value = 1);
    target.with_by_handle_mut(replace, |links| links.value = 2);

    let source = LinkStore::<&'static str, Links>::default();
    let source_replace = source.ensure_handle("replace");
    let source_new = source.ensure_handle("new");
    source.with_by_handle_mut(source_replace, |links| links.value = 3);
    source.with_by_handle_mut(source_new, |links| links.value = 4);

    target.extend_from(source);

    assert_eq!(
        target.with_by_handle(target.try_handle("keep").unwrap(), |links| links.value),
        1
    );
    assert_eq!(
        target.with_by_handle(target.try_handle("replace").unwrap(), |links| links.value),
        3
    );
    assert_eq!(
        target.with_by_handle(target.try_handle("new").unwrap(), |links| links.value),
        4
    );
}

#[test]
fn handles_access_existing_entries_without_relooking_up_keys() {
    let links = LinkStore::<&'static str, Links>::default();

    assert!(links.try_handle("key").is_none());
    let handle = links.ensure_handle("key");
    links.with_by_handle_mut(handle, |links| links.value = 7);

    assert_eq!(links.try_handle("key"), Some(handle));
    assert_eq!(links.with_by_handle(handle, |links| links.value), 7);
    assert_eq!(
        links.with_by_handle(links.try_handle("key").unwrap(), |links| links.value),
        7
    );
}

#[cfg(feature = "link_store_stats")]
#[test]
fn link_store_stats_count_enabled_handle_accesses() {
    reset_link_store_stats();
    set_link_store_stats_enabled(true);

    let links = LinkStore::<&'static str, Links>::default();
    let handle = links.ensure_handle("key");
    let same_handle = links.ensure_handle("key");
    assert_eq!(same_handle, handle);
    links.with_by_handle(handle, |links| links.value);
    links.with_by_handle_mut(handle, |links| links.value = 1);

    set_link_store_stats_enabled(false);
    let stats = link_store_stats_snapshot();
    reset_link_store_stats();

    assert_eq!(stats.ensure_handle, 2);
    assert_eq!(stats.ensure_handle_hit, 1);
    assert_eq!(stats.ensure_handle_miss, 1);
    assert_eq!(stats.with_by_handle, 1);
    assert_eq!(stats.with_by_handle_mut, 1);
}
