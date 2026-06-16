use super::*;
use std::collections::HashMap;
use ts_vfs::Fs;

#[test]
fn test_concurrent_duration_events_use_separate_thread_ids() {
    let fs = ts_vfs::vfstest::from_map([("/trace", directory())], true);

    let mut tr = start_tracing(fs.clone(), "/trace", "", true).unwrap();
    let end_a = tr.push(Phase::Parse, "createSourceFile", [("path", "/a.ts")], true);
    let end_b = tr.push(Phase::Parse, "createSourceFile", [("path", "/b.ts")], true);
    end_a(&mut tr);
    end_b(&mut tr);

    let end_check = tr.push(
        Phase::Check,
        "checkSourceFile",
        HashMap::from([("checkerId", Any::from(0)), ("path", Any::from("/a.ts"))]),
        true,
    );
    let end_variance = tr.push(
        Phase::CheckTypes,
        "getVariancesWorker",
        HashMap::from([("checkerId", Any::from(0)), ("id", Any::from(1))]),
        true,
    );
    end_variance(&mut tr);
    end_check(&mut tr);

    tr.stop_tracing().unwrap();

    let (trace_text, ok) = fs.read_file("/trace/trace.json");
    assert!(ok);
    let events: Vec<TraceEvent> = serde_json::from_str(&trace_text).unwrap();

    let a_begin = find_event(&events, "B", "createSourceFile", "path", "/a.ts");
    let a_end = find_event(&events, "E", "createSourceFile", "path", "/a.ts");
    let b_begin = find_event(&events, "B", "createSourceFile", "path", "/b.ts");
    let b_end = find_event(&events, "E", "createSourceFile", "path", "/b.ts");
    assert_eq!(a_begin.tid, a_end.tid);
    assert_eq!(b_begin.tid, b_end.tid);
    assert_ne!(a_begin.tid, b_begin.tid);
    assert_thread_name(&events, a_begin.tid, "file:/a.ts");
    assert_thread_name(&events, b_begin.tid, "file:/b.ts");

    let check_begin = find_event(&events, "B", "checkSourceFile", "path", "/a.ts");
    let variance_begin = find_event(&events, "B", "getVariancesWorker", "id", 1);
    assert_eq!(check_begin.tid, variance_begin.tid);
    assert_thread_name(&events, check_begin.tid, "checker:0");

    assert_duration_events_are_well_nested_by_thread(&events);
}

#[test]
fn test_thread_ids_are_stable_across_first_seen_order() {
    let first = trace_thread_ids_for_paths(["/a.ts", "/b.ts"]);
    let second = trace_thread_ids_for_paths(["/b.ts", "/a.ts"]);
    assert_eq!(first, second);
}

fn directory() -> ts_vfs::vfstest::MapFile {
    ts_vfs::vfstest::MapFile {
        mode: ts_vfs::FileType::directory(),
        ..Default::default()
    }
}

fn trace_thread_ids_for_paths<const N: usize>(paths: [&str; N]) -> HashMap<String, i32> {
    let fs = ts_vfs::vfstest::from_map([("/trace", directory())], true);
    let mut tr = start_tracing(fs.clone(), "/trace", "", true).unwrap();

    for path in paths {
        let end = tr.push(Phase::Parse, "createSourceFile", [("path", path)], true);
        end(&mut tr);
    }

    tr.stop_tracing().unwrap();

    let (trace_text, ok) = fs.read_file("/trace/trace.json");
    assert!(ok);
    let events: Vec<TraceEvent> = serde_json::from_str(&trace_text).unwrap();

    paths
        .into_iter()
        .map(|path| {
            (
                path.to_string(),
                find_event(&events, "B", "createSourceFile", "path", path).tid,
            )
        })
        .collect()
}

fn find_event<'a>(
    events: &'a [TraceEvent],
    phase: &str,
    name: &str,
    arg_name: &str,
    arg_value: impl Into<Any>,
) -> &'a TraceEvent {
    let arg_value = arg_value.into();
    events
        .iter()
        .find(|event| {
            event.ph == phase
                && event.name == name
                && event
                    .args
                    .get(arg_name)
                    .is_some_and(|value| value == &arg_value)
        })
        .unwrap_or_else(|| {
            panic!("failed to find {phase} event {name:?} with {arg_name}={arg_value}")
        })
}

fn assert_thread_name(events: &[TraceEvent], tid: i32, name: &str) {
    assert!(events.iter().any(|event| {
        event.ph == "M"
            && event.name == "thread_name"
            && event.tid == tid
            && event.args.get("name").is_some_and(|value| value == name)
    }));
}

fn assert_duration_events_are_well_nested_by_thread(events: &[TraceEvent]) {
    let mut stacks: HashMap<i32, Vec<&TraceEvent>> = HashMap::new();
    for event in events {
        match event.ph.as_str() {
            "B" => stacks.entry(event.tid).or_default().push(event),
            "E" => {
                let stack = stacks.get_mut(&event.tid).unwrap_or_else(|| {
                    panic!(
                        "unmatched end event {:?} on thread {}",
                        event.name, event.tid
                    )
                });
                let begin = stack.pop().unwrap_or_else(|| {
                    panic!(
                        "unmatched end event {:?} on thread {}",
                        event.name, event.tid
                    )
                });
                assert_eq!(begin.cat, event.cat);
                assert_eq!(begin.name, event.name);
            }
            _ => {}
        }
    }
    for (tid, stack) in stacks {
        assert!(stack.is_empty(), "thread {tid} has unterminated events");
    }
}
