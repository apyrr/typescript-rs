use std::{sync::Mutex, time::Duration};

use ts_diagnostics as diagnostics;

use crate::{ProgressArg, ProgressReporter, lsproto, new_project_loading_progress_from_reporter};

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProgressCall {
    method: String,
    token: String,
    title: String,
    msg: String,
}

struct FakeProgressReporter {
    calls: Mutex<Vec<ProgressCall>>,
    done_tx: tokio::sync::watch::Sender<bool>,
}

impl FakeProgressReporter {
    fn new() -> std::sync::Arc<Self> {
        let (done_tx, _done_rx) = tokio::sync::watch::channel(false);
        std::sync::Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            done_tx,
        })
    }

    fn cancel(&self) {
        let _ = self.done_tx.send(true);
    }

    fn get_calls(&self) -> Vec<ProgressCall> {
        self.calls
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }
}

impl ProgressReporter for FakeProgressReporter {
    fn done(&self) -> tokio::sync::watch::Receiver<bool> {
        self.done_tx.subscribe()
    }

    fn localize(&self, msg: &diagnostics::Message, args: Vec<ProgressArg>) -> String {
        match msg.key().as_str() {
            "Loading_100012" => "Loading".to_string(),
            "Project_0_100014" => {
                let arg = args
                    .first()
                    .and_then(|arg| arg.as_any().downcast_ref::<String>())
                    .cloned()
                    .unwrap_or_default();
                format!("Project '{arg}'")
            }
            _ => msg.string(),
        }
    }

    fn create_work_done_progress(&self, token: String) {
        self.calls
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(ProgressCall {
                method: "create".to_string(),
                token,
                title: String::new(),
                msg: String::new(),
            });
    }

    fn send_progress(&self, token: String, value: lsproto::WorkDoneProgressBeginOrReportOrEnd) {
        let mut calls = self.calls.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(begin) = value.begin {
            calls.push(ProgressCall {
                method: "begin".to_string(),
                token,
                title: begin.title,
                msg: begin.message.unwrap_or_default(),
            });
        } else if let Some(report) = value.report {
            calls.push(ProgressCall {
                method: "report".to_string(),
                token,
                title: String::new(),
                msg: report.message.unwrap_or_default(),
            });
        } else if value.end.is_some() {
            calls.push(ProgressCall {
                method: "end".to_string(),
                token,
                title: String::new(),
                msg: String::new(),
            });
        }
    }
}

fn string_arg(value: &str) -> Vec<ProgressArg> {
    vec![Box::new(value.to_string()) as ProgressArg]
}

async fn wait() {
    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
}

#[tokio::test]
async fn test_progress_start_finish_before_delay() {
    let reporter = FakeProgressReporter::new();
    let p =
        new_project_loading_progress_from_reporter(reporter.clone(), Duration::from_millis(500));

    p.start(&diagnostics::Project_0, string_arg("myProject"))
        .await;
    wait().await;

    // Finish before the delay fires; no UI should appear.
    p.finish(&diagnostics::Project_0, string_arg("myProject"))
        .await;
    wait().await;

    // Advance time past the delay to ensure no progress is sent.
    tokio::time::sleep(Duration::from_millis(600)).await;
    wait().await;

    let calls = reporter.get_calls();
    assert!(
        calls.is_empty(),
        "expected no progress calls for fast operation, got {calls:?}"
    );

    reporter.cancel();
}

#[tokio::test]
async fn test_progress_shows_after_delay() {
    let reporter = FakeProgressReporter::new();
    let p =
        new_project_loading_progress_from_reporter(reporter.clone(), Duration::from_millis(500));

    p.start(&diagnostics::Project_0, string_arg("myProject"))
        .await;
    wait().await;

    // Let the delay fire.
    tokio::time::sleep(Duration::from_millis(500)).await;
    wait().await;

    let mut calls = reporter.get_calls();
    assert_eq!(
        calls.len(),
        2,
        "expected 2 calls (create + begin), got {}: {calls:?}",
        calls.len()
    );
    assert_eq!(
        calls[0].method, "create",
        "expected create, got {:?}",
        calls[0]
    );
    assert_eq!(
        calls[1].method, "begin",
        "expected begin, got {:?}",
        calls[1]
    );
    assert_eq!(
        calls[1].title,
        diagnostics::Loading.string(),
        "expected title {:?}, got {:?}",
        diagnostics::Loading.string(),
        calls[1].title
    );

    // Finish the operation.
    p.finish(&diagnostics::Project_0, string_arg("myProject"))
        .await;
    wait().await;

    calls = reporter.get_calls();
    let last = calls.last().expect("expected progress calls");
    assert_eq!(last.method, "end", "expected end, got {last:?}");

    reporter.cancel();
}

#[tokio::test]
async fn test_progress_reports_multiple_operations() {
    let reporter = FakeProgressReporter::new();
    let p =
        new_project_loading_progress_from_reporter(reporter.clone(), Duration::from_millis(100));

    // Start two different operations.
    p.start(&diagnostics::Project_0, string_arg("projA")).await;
    p.start(&diagnostics::Project_0, string_arg("projB")).await;
    wait().await;

    // Let the delay fire.
    tokio::time::sleep(Duration::from_millis(100)).await;
    wait().await;

    let mut calls = reporter.get_calls();
    // Should have: create, begin (with first message).
    assert!(
        calls.len() >= 2,
        "expected at least 2 calls, got {}: {calls:?}",
        calls.len()
    );
    assert_eq!(
        calls[0].method, "create",
        "expected create, got {:?}",
        calls[0]
    );
    assert_eq!(
        calls[1].method, "begin",
        "expected begin, got {:?}",
        calls[1]
    );

    // Finish one; should send a report with the remaining operation.
    p.finish(&diagnostics::Project_0, string_arg("projA")).await;
    wait().await;

    calls = reporter.get_calls();
    let found = calls.iter().any(|call| call.method == "report");
    assert!(
        found,
        "expected a report after partial finish, got {calls:?}"
    );

    // Finish the second; should send end.
    p.finish(&diagnostics::Project_0, string_arg("projB")).await;
    wait().await;

    calls = reporter.get_calls();
    let last = calls.last().expect("expected progress calls");
    assert_eq!(last.method, "end", "expected end, got {last:?}");

    reporter.cancel();
}

#[tokio::test]
async fn test_progress_ref_counting() {
    let reporter = FakeProgressReporter::new();
    let p =
        new_project_loading_progress_from_reporter(reporter.clone(), Duration::from_millis(100));

    // Start the same operation twice (ref count = 2).
    p.start(&diagnostics::Project_0, string_arg("proj")).await;
    p.start(&diagnostics::Project_0, string_arg("proj")).await;
    wait().await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    wait().await;

    // Finish once (ref count = 1); should NOT end.
    p.finish(&diagnostics::Project_0, string_arg("proj")).await;
    wait().await;

    let mut calls = reporter.get_calls();
    assert!(
        calls.iter().all(|call| call.method != "end"),
        "unexpected end with ref count > 0: {calls:?}"
    );

    // Finish again (ref count = 0); should end.
    p.finish(&diagnostics::Project_0, string_arg("proj")).await;
    wait().await;

    calls = reporter.get_calls();
    let last = calls.last().expect("expected progress calls");
    assert_eq!(
        last.method, "end",
        "expected end when ref count reaches 0, got {last:?}"
    );

    reporter.cancel();
}

#[tokio::test]
async fn test_progress_new_token_after_end() {
    let reporter = FakeProgressReporter::new();
    let p =
        new_project_loading_progress_from_reporter(reporter.clone(), Duration::from_millis(100));

    // First cycle.
    p.start(&diagnostics::Project_0, string_arg("proj")).await;
    wait().await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    wait().await;

    let mut calls = reporter.get_calls();
    let first_token = calls[0].token.clone();

    p.finish(&diagnostics::Project_0, string_arg("proj")).await;
    wait().await;

    // Second cycle; should get a new token.
    p.start(&diagnostics::Project_0, string_arg("proj2")).await;
    wait().await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    wait().await;

    calls = reporter.get_calls();
    let second_token = calls
        .iter()
        .find(|call| call.method == "create" && call.token != first_token)
        .map(|call| call.token.clone())
        .unwrap_or_default();
    assert!(
        !second_token.is_empty(),
        "expected a new token for second cycle, got calls: {calls:?}"
    );
    assert_ne!(
        first_token, second_token,
        "expected different tokens, both were {first_token:?}"
    );

    p.finish(&diagnostics::Project_0, string_arg("proj2")).await;
    wait().await;

    reporter.cancel();
}

#[tokio::test]
async fn test_progress_start_before_delay_then_more_after_delay() {
    let reporter = FakeProgressReporter::new();
    let p =
        new_project_loading_progress_from_reporter(reporter.clone(), Duration::from_millis(200));

    // Start before delay.
    p.start(&diagnostics::Project_0, string_arg("projA")).await;
    wait().await;

    // Let delay fire.
    tokio::time::sleep(Duration::from_millis(200)).await;
    wait().await;

    let mut calls = reporter.get_calls();
    assert!(
        calls.len() >= 2,
        "expected create + begin after delay, got {calls:?}"
    );

    // Start another operation after delay; should send a report immediately.
    p.start(&diagnostics::Project_0, string_arg("projB")).await;
    wait().await;

    calls = reporter.get_calls();
    let last = calls.last().expect("expected progress calls");
    assert_eq!(
        last.method, "report",
        "expected report for new start after delay, got {last:?}"
    );

    // Clean up.
    p.finish(&diagnostics::Project_0, string_arg("projA")).await;
    p.finish(&diagnostics::Project_0, string_arg("projB")).await;
    wait().await;

    reporter.cancel();
}

#[tokio::test]
async fn test_progress_finish_with_no_active_token() {
    let reporter = FakeProgressReporter::new();
    let p =
        new_project_loading_progress_from_reporter(reporter.clone(), Duration::from_millis(100));

    // Finish without any prior start; should be a no-op.
    p.finish(&diagnostics::Project_0, string_arg("proj")).await;
    wait().await;

    let calls = reporter.get_calls();
    assert!(
        calls.is_empty(),
        "expected no calls for orphan finish, got {calls:?}"
    );

    reporter.cancel();
}

#[tokio::test]
async fn test_progress_shutdown_during_start_and_finish() {
    let reporter = FakeProgressReporter::new();
    let p =
        new_project_loading_progress_from_reporter(reporter.clone(), Duration::from_millis(100));

    // Cancel context so the run task exits.
    reporter.cancel();
    wait().await;

    // These should return immediately via the done() path since the context is cancelled.
    p.start(&diagnostics::Project_0, string_arg("proj")).await;
    p.finish(&diagnostics::Project_0, string_arg("proj")).await;
}

#[tokio::test]
async fn test_progress_shutdown_with_active_timer() {
    let reporter = FakeProgressReporter::new();
    let p =
        new_project_loading_progress_from_reporter(reporter.clone(), Duration::from_millis(500));

    // Start an operation so the delay timer is created.
    p.start(&diagnostics::Project_0, string_arg("proj")).await;
    wait().await;

    // Shutdown while the delay timer is still pending.
    reporter.cancel();
    wait().await;
}

#[tokio::test]
async fn test_progress_zero_delay() {
    let reporter = FakeProgressReporter::new();
    let p = new_project_loading_progress_from_reporter(reporter.clone(), Duration::ZERO);

    // With zero delay, progress should begin immediately.
    p.start(&diagnostics::Project_0, string_arg("proj")).await;
    wait().await;

    let mut calls = reporter.get_calls();
    assert_eq!(
        calls.len(),
        2,
        "expected 2 calls (create + begin), got {}: {calls:?}",
        calls.len()
    );
    assert_eq!(
        calls[0].method, "create",
        "expected create, got {:?}",
        calls[0]
    );
    assert_eq!(
        calls[1].method, "begin",
        "expected begin, got {:?}",
        calls[1]
    );
    assert_eq!(
        calls[1].msg, "Project 'proj'",
        "expected message {:?}, got {:?}",
        "Project 'proj'", calls[1].msg
    );

    // Start+finish should still produce begin and end.
    p.finish(&diagnostics::Project_0, string_arg("proj")).await;
    wait().await;

    calls = reporter.get_calls();
    let last = calls.last().expect("expected progress calls");
    assert_eq!(last.method, "end", "expected end, got {last:?}");

    reporter.cancel();
}

#[tokio::test]
async fn test_progress_finish_before_delay_no_begun() {
    let reporter = FakeProgressReporter::new();
    let p =
        new_project_loading_progress_from_reporter(reporter.clone(), Duration::from_millis(500));

    // Start, then finish before delay; begun is false, so no end is sent.
    p.start(&diagnostics::Project_0, string_arg("proj")).await;
    wait().await;
    p.finish(&diagnostics::Project_0, string_arg("proj")).await;
    wait().await;

    let calls = reporter.get_calls();
    assert!(
        calls.iter().all(|call| call.method != "end"),
        "unexpected end when begun=false: {calls:?}"
    );

    reporter.cancel();
}
