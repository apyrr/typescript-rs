use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
};

use super::{SyncMapEntry, new_sync_map};

#[derive(Clone, Default)]
struct TestValue {
    data: String,
}

#[test]
fn test_sync_map_proxy_for() {
    // Keep the Go subtest structure in one Rust test so the source ordering stays obvious.

    // "proxy for race condition"
    {
        let mut base = HashMap::new();
        base.insert(
            "key1".to_string(),
            TestValue {
                data: "original".to_string(),
            },
        );
        let sync_map = Arc::new(new_sync_map(base));

        let sync_map_1 = Arc::clone(&sync_map);
        let handle1 = thread::spawn(move || sync_map_1.load("key1".to_string()).unwrap());

        let sync_map_2 = Arc::clone(&sync_map);
        let handle2 = thread::spawn(move || sync_map_2.load("key1".to_string()).unwrap());

        let entry1 = handle1.join().unwrap();
        let entry2 = handle2.join().unwrap();

        assert_eq!(
            "original",
            entry1
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .value()
                .data
        );
        assert_eq!(
            "original",
            entry2
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .value()
                .data
        );
        assert!(!entry1.lock().unwrap_or_else(|err| err.into_inner()).dirty());
        assert!(!entry2.lock().unwrap_or_else(|err| err.into_inner()).dirty());

        let entry1_change = Arc::clone(&entry1);
        let change1 = thread::spawn(move || {
            entry1_change
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .change(|v| {
                    v.data = "changed_by_entry1".to_string();
                });
        });

        let entry2_change = Arc::clone(&entry2);
        let change2 = thread::spawn(move || {
            entry2_change
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .change(|v| {
                    v.data = "changed_by_entry2".to_string();
                });
        });

        change1.join().unwrap();
        change2.join().unwrap();

        let final_value1 = entry1
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .value()
            .data;
        let final_value2 = entry2
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .value()
            .data;
        assert_eq!(final_value1, final_value2);

        assert!(entry1.lock().unwrap_or_else(|err| err.into_inner()).dirty());
        assert!(entry2.lock().unwrap_or_else(|err| err.into_inner()).dirty());

        let has_proxy = entry1
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .proxy_for
            .is_some()
            || entry2
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .proxy_for
                .is_some();
        assert!(has_proxy);
    }

    // "proxy operations delegation"
    {
        let mut base = HashMap::new();
        base.insert(
            "key1".to_string(),
            TestValue {
                data: "original".to_string(),
            },
        );
        let sync_map = new_sync_map(base);

        let entry1 = sync_map.load("key1".to_string()).unwrap();
        let entry2 = sync_map.load("key1".to_string()).unwrap();

        entry1
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .change(|v| {
                v.data = "changed_by_entry1".to_string();
            });
        entry2
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .change(|v| {
                v.data = "changed_by_entry2".to_string();
            });

        let (proxy, target): (
            Arc<Mutex<SyncMapEntry<String, TestValue>>>,
            Arc<Mutex<SyncMapEntry<String, TestValue>>>,
        ) = if entry1
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .proxy_for
            .is_some()
        {
            (Arc::clone(&entry1), Arc::clone(&entry2))
        } else {
            (Arc::clone(&entry2), Arc::clone(&entry1))
        };

        proxy
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .change(|v| {
                v.data = "changed_through_proxy".to_string();
            });
        assert_eq!(
            "changed_through_proxy",
            target
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .value()
                .data
        );
        assert_eq!(
            "changed_through_proxy",
            proxy
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .value()
                .data
        );

        let changed = proxy
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .change_if(
                |v| v.data == "changed_through_proxy",
                |v| {
                    v.data = "conditional_change".to_string();
                },
            );
        assert!(changed);
        assert_eq!(
            "conditional_change",
            target
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .value()
                .data
        );
        assert_eq!(
            "conditional_change",
            proxy
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .value()
                .data
        );

        let target_dirty = target.lock().unwrap_or_else(|err| err.into_inner()).dirty();
        let proxy_dirty = proxy.lock().unwrap_or_else(|err| err.into_inner()).dirty();
        assert_eq!(target_dirty, proxy_dirty);

        proxy
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .locked(|v| {
                v.change(|val| {
                    val.data = "locked_change".to_string();
                });
            });
        assert_eq!(
            "locked_change",
            target
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .value()
                .data
        );
        assert_eq!(
            "locked_change",
            proxy
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .value()
                .data
        );
    }

    // "proxy delete operations"
    {
        let mut base = HashMap::new();
        base.insert(
            "key1".to_string(),
            TestValue {
                data: "original".to_string(),
            },
        );
        let sync_map = new_sync_map(base);

        let entry1 = sync_map.load("key1".to_string()).unwrap();
        let entry2 = sync_map.load("key1".to_string()).unwrap();

        entry1
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .change(|v| v.data = "modified".to_string());
        entry2
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .change(|v| v.data = "modified2".to_string());

        let proxy = if entry1
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .proxy_for
            .is_some()
        {
            Arc::clone(&entry1)
        } else {
            Arc::clone(&entry2)
        };

        proxy.lock().unwrap_or_else(|err| err.into_inner()).delete();

        assert!(sync_map.load("key1".to_string()).is_none());

        let mut base2 = HashMap::new();
        base2.insert(
            "key2".to_string(),
            TestValue {
                data: "test".to_string(),
            },
        );
        let sync_map2 = new_sync_map(base2);

        let entry3 = sync_map2.load("key2".to_string()).unwrap();
        let entry4 = sync_map2.load("key2".to_string()).unwrap();

        entry3
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .change(|v| v.data = "modified".to_string());
        entry4
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .change(|v| v.data = "modified2".to_string());

        let proxy2 = if entry3
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .proxy_for
            .is_some()
        {
            Arc::clone(&entry3)
        } else {
            Arc::clone(&entry4)
        };

        proxy2
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .delete_if(|v| v.data == "modified2" || v.data == "modified");

        assert!(sync_map2.load("key2".to_string()).is_none());
    }

    // "no proxy when no race"
    {
        let mut base = HashMap::new();
        base.insert(
            "key1".to_string(),
            TestValue {
                data: "original".to_string(),
            },
        );
        let sync_map = new_sync_map(base);

        let entry = sync_map.load("key1".to_string()).unwrap();
        entry
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .change(|v| {
                v.data = "changed".to_string();
            });

        assert!(
            entry
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .proxy_for
                .is_none()
        );
        assert!(entry.lock().unwrap_or_else(|err| err.into_inner()).dirty());
        assert_eq!(
            "changed",
            entry
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .value()
                .data
        );
    }
}
