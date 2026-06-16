use super::{Arena, ArenaMap, Idx, RawIdx};

#[derive(Debug, PartialEq, Eq)]
struct Node;

#[derive(Debug, PartialEq, Eq)]
struct Symbol;

#[test]
fn idx_is_copyable_and_typed() {
    let node_idx = Idx::<Node>::from_raw(RawIdx::from_u32(7));
    let copied = node_idx;

    assert_eq!(node_idx, copied);
    let _: Idx<Node> = copied;
    let _: Idx<Symbol> = Idx::from_raw(RawIdx::from_u32(7));
}

#[test]
fn arena_allocation_and_indexing_are_stable() {
    let mut arena = Arena::new();

    let first = arena.alloc("first");
    let second = arena.alloc("second");
    let range = arena.alloc_many(["third", "fourth"]);

    assert_eq!(arena[first], "first");
    assert_eq!(arena[second], "second");
    assert_eq!(arena.get(first), Some(&"first"));
    *arena.get_mut(second).unwrap() = "second-updated";
    assert_eq!(arena[second], "second-updated");
    assert_eq!(&arena[range], &["third", "fourth"]);
    assert_eq!(
        arena
            .iter()
            .map(|(idx, value)| (idx.into_raw().into_u32(), *value))
            .collect::<Vec<_>>(),
        vec![
            (0, "first"),
            (1, "second-updated"),
            (2, "third"),
            (3, "fourth"),
        ]
    );
}

#[test]
fn arena_map_insert_get_remove_and_iterate_dense_ids() {
    let mut arena = Arena::new();
    let first = arena.alloc(Node);
    let second = arena.alloc(Node);
    let third = arena.alloc(Node);

    let mut map = ArenaMap::new();
    assert_eq!(map.insert(first, "a"), None);
    assert_eq!(map.insert(third, "c"), None);
    assert_eq!(map.insert(first, "updated"), Some("a"));

    assert_eq!(map.get(first), Some(&"updated"));
    assert_eq!(map.get(second), None);
    assert_eq!(map[third], "c");
    assert!(map.contains_idx(first));

    assert_eq!(
        map.iter()
            .map(|(idx, value)| (idx.into_raw().into_u32(), *value))
            .collect::<Vec<_>>(),
        vec![(0, "updated"), (2, "c")]
    );
    assert_eq!(map.remove(first), Some("updated"));
    assert!(!map.contains_idx(first));
}

#[test]
fn arena_map_into_iter_skips_empty_slots_from_both_ends() {
    let mut arena = Arena::new();
    let first = arena.alloc(Node);
    let _second = arena.alloc(Node);
    let third = arena.alloc(Node);

    let mut map = ArenaMap::new();
    map.insert(first, 10);
    map.insert(third, 30);

    let mut iter = map.into_iter();
    assert_eq!(
        iter.next()
            .map(|(idx, value)| (idx.into_raw().into_u32(), value)),
        Some((0, 10))
    );
    assert_eq!(
        iter.next_back()
            .map(|(idx, value)| (idx.into_raw().into_u32(), value)),
        Some((2, 30))
    );
    assert_eq!(iter.next(), None);
}
