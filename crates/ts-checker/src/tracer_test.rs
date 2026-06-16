use std::collections::HashMap;

use serde::Deserialize;
use ts_json as json;
use ts_tracing as tracing;
use ts_vfs::vfstest;

use crate::tracer::new_tracer;

#[test]
fn test_tracer_push_preserves_end_arg_mutations() {
    let fsys = vfstest::from_map(
        [(
            "/trace",
            vfstest::MapFile {
                mode: vfstest::MODE_DIR, ..Default::default()
            },
        )],
        true,
    );

    let mut tr =
        tracing::start_tracing(fsys.clone(), "/trace", "", true /*deterministic*/).unwrap();

    let mut args = HashMap::from([("id".to_string(), 1.into())]);
    let mut tracer = new_tracer(&mut tr, 7);
    let pop = tracer.push(
        tracing::Phase::CheckTypes,
        "getVariancesWorker",
        &mut args,
        true,
    );
    let has_checker_id = args.contains_key("checkerId");
    assert!(!has_checker_id);

    args.insert("variances".to_string(), vec!["out"].into());
    pop();
    let has_checker_id = args.contains_key("checkerId");
    assert!(!has_checker_id);

    tr.stop_tracing().unwrap();

    let trace_text = fsys.read_file("/trace/trace.json").unwrap();

    let events: Vec<TestTraceEvent> = json::unmarshal(trace_text.as_bytes()).unwrap();

    let begin_event = find_test_trace_event(&events, "B", "getVariancesWorker");
    assert_eq!(begin_event.args.get("checkerId"), Some(&7.0.into()));
    assert_eq!(begin_event.args.get("variances"), None);

    let end_event = find_test_trace_event(&events, "E", "getVariancesWorker");
    assert_eq!(end_event.args.get("checkerId"), Some(&7.0.into()));
    let variances = end_event.args.get("variances").unwrap().as_array().unwrap();
    assert_eq!(variances, &vec!["out".into()]);
}

#[derive(Deserialize)]
struct TestTraceEvent {
    #[serde(rename = "ph")]
    ph: String,
    name: String,
    args: HashMap<String, serde_json::Value>,
}

fn find_test_trace_event(events: &[TestTraceEvent], phase: &str, name: &str) -> &TestTraceEvent {
    for event in events {
        if event.ph == phase && event.name == name {
            return event;
        }
    }
    panic!("failed to find {} event {:?}", phase, name);
}

