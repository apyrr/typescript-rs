use crate::{OrderedMap, new_ordered_map_with_size_hint};

#[test]
fn test_ordered_map() {
    let mut m = OrderedMap::<i32, String>::default();

    assert!(!m.has(&1));

    const N: i32 = 1000;
    const START: i32 = 1;
    const END: i32 = START + N;

    // Seed the map with ascending keys and values for easier testing.
    for i in START..END {
        m.set(i, pad_int(i));
    }

    assert_eq!(m.size(), N as usize);

    // Attempt to overwrite existing keys in reverse order.
    for i in (START..END).rev() {
        m.set(i, pad_int(i));
    }

    assert_eq!(m.size(), N as usize);

    for i in START..END {
        let v = m.get(&i);
        assert!(v.is_some());
        assert_eq!(v.unwrap(), &pad_int(i));
    }

    for (k, v) in m.entries() {
        assert_eq!(v, &pad_int(*k));
    }

    let keys = m.keys().copied().collect::<Vec<_>>();
    assert_eq!(keys.len(), N as usize);
    assert!(keys.windows(2).all(|pair| pair[0] <= pair[1]));

    let values = m.values().cloned().collect::<Vec<_>>();
    assert_eq!(values.len(), N as usize);
    assert!(values.windows(2).all(|pair| pair[0] <= pair[1]));

    let mut first_key = 0;
    for k in m.keys() {
        first_key = *k;
        break;
    }
    assert_eq!(first_key, START);

    let mut first_value = String::new();
    for v in m.values() {
        first_value = v.clone();
        break;
    }
    assert_eq!(first_value, pad_int(START));

    for (k, v) in m.entries() {
        first_key = *k;
        first_value = v.clone();
        break;
    }

    assert_eq!(first_key, START);
    assert_eq!(first_value, pad_int(START));

    for i in START + 1..END {
        let v = m.delete(&i);
        assert!(v.is_some());
        assert_eq!(v.unwrap(), pad_int(i));
        assert!(!m.has(&i));

        let v = m.get(&i);
        assert!(v.is_none());

        let v = m.delete(&i);
        assert!(v.is_none());
    }

    assert_eq!(m.size(), 1);
    assert!(m.has(&START));

    let v = m.delete(&START);
    assert!(v.is_some());
    assert_eq!(v.unwrap(), pad_int(START));

    assert_eq!(m.size(), 0);
}

#[test]
fn test_ordered_map_clone() {
    let mut m = OrderedMap::<i32, String>::default();
    m.set(1, "one".to_string());
    m.set(2, "two".to_string());

    let clone = m.clone();

    assert!(!std::ptr::eq(&clone, &m));
    assert_eq!(clone.size(), 2);
    assert_eq!(clone.keys().copied().collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(
        clone.values().cloned().collect::<Vec<_>>(),
        vec!["one".to_string(), "two".to_string()]
    );

    let v = clone.get(&1);
    assert!(v.is_some());
    assert_eq!(v.unwrap(), "one");

    m.delete(&1);

    assert_eq!(m.size(), 1);
    assert_eq!(clone.size(), 2);
    assert_eq!(clone.keys().copied().collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(
        clone.values().cloned().collect::<Vec<_>>(),
        vec!["one".to_string(), "two".to_string()]
    );
}

#[test]
fn test_ordered_map_clear() {
    let mut m = OrderedMap::<i32, String>::default();
    m.set(1, "one".to_string());
    m.set(2, "two".to_string());

    m.clear();

    assert_eq!(m.size(), 0);
}

fn pad_int(n: i32) -> String {
    format!("{n:10}")
}

#[test]
fn test_ordered_map_with_size_hint() {
    const N: usize = 1024;

    let mut m = new_ordered_map_with_size_hint(N);
    for i in 0..N {
        m.set(i, i);
    }

    assert_eq!(m.size(), N);
}

#[test]
fn test_ordered_map_unmarshal_json() {
    test_ordered_map_unmarshal_json_with(|text, out| out.unmarshal_json_from(text));
}

fn test_ordered_map_unmarshal_json_with(
    unmarshal: impl Fn(
        &str,
        &mut OrderedMap<String, serde_json::Value>,
    ) -> Result<(), Box<dyn std::error::Error>>,
) {
    let mut m = OrderedMap::<String, serde_json::Value>::default();
    unmarshal(r#"{"a": 1, "b": "two", "c": { "d": 4 } }"#, &mut m).unwrap();

    assert_eq!(m.size(), 3);
    assert_eq!(m.get_or_zero(&"a".to_string()), serde_json::json!(1));

    unmarshal("null", &mut m).unwrap();

    let invalid = unmarshal(r#""foo""#, &mut m).unwrap_err();
    assert!(
        invalid
            .to_string()
            .contains("cannot unmarshal non-object JSON value into Map"),
        "err = {invalid}"
    );

    let mut invalid_map = OrderedMap::<i32, serde_json::Value>::default();
    let invalid_map = invalid_map
        .unmarshal_json_from(r#"{"a": 1, "b": "two"}"#)
        .unwrap_err();
    assert!(
        invalid_map.to_string().contains("unmarshal"),
        "err = {invalid_map}"
    );
}
