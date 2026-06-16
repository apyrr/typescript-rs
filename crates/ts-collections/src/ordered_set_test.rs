use crate::{OrderedSet, new_ordered_set_with_size_hint};

#[test]
fn ordered_set() {
    let mut s = OrderedSet::<i32>::default();

    s.add(1);
    s.add(2);
    s.add(3);

    assert!(s.has(&1));
    assert!(s.has(&2));
    assert!(s.has(&3));

    assert!(s.delete(&2));

    let values = s.values().copied().collect::<Vec<_>>();
    assert_eq!(values.len(), 2);
    assert!(values.windows(2).all(|pair| pair[0] <= pair[1]));

    s.clear();

    assert_eq!(s.size(), 0);
    assert!(!s.has(&1));
    assert!(!s.has(&2));
    assert!(!s.has(&3));

    let s2 = s.clone_set();
    assert!(!std::ptr::eq(&s, &s2));
    assert_eq!(s2.size(), 0);
}

#[test]
fn ordered_set_with_size_hint() {
    const N: usize = 1024;

    let mut m = new_ordered_set_with_size_hint(N);
    let (values_capacity, set_capacity) = m.capacities();
    assert!(values_capacity >= N);
    assert!(set_capacity >= N);

    for i in 0..N {
        m.add(i);
    }
    assert_eq!(m.size(), N);
}
