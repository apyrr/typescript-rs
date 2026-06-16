use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, atomic::AtomicI32},
    time::Duration,
};

use ts_collections as collections;
use ts_core as core;
use ts_diagnostics as diagnostics;

use ts_core::context;
use ts_locale as locale;

use crate::{lsproto, server::Server};

pub type ProgressArg = Box<dyn diagnostics::DiagnosticArg + Send + Sync>;

pub struct ProgressEvent {
    pub message: Option<diagnostics::Message>,
    pub args: Vec<ProgressArg>,
    pub finish: bool,
}

pub trait ProgressReporter {
    // done returns a channel that is closed when the server is shutting down.
    fn done(&self) -> tokio::sync::watch::Receiver<bool>;
    // localize converts a diagnostic message to a display string.
    fn localize(&self, msg: &diagnostics::Message, args: Vec<ProgressArg>) -> String;
    // createWorkDoneProgress asks the client to create a progress token.
    fn create_work_done_progress(&self, token: String);
    // sendProgress sends a $/progress notification.
    fn send_progress(&self, token: String, value: lsproto::WorkDoneProgressBeginOrReportOrEnd);
}

pub fn spawn_progress_task<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(future);
    } else {
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("progress runtime should build");
            runtime.block_on(future);
        });
    }
}

// serverProgressReporter adapts *Server to the progressReporter interface.
pub struct ServerProgressReporter {
    pub background_ctx: context::Context,
    pub locale: locale::Locale,
    pub client_seq: Arc<AtomicI32>,
    pub outgoing_queue: Arc<Mutex<Vec<lsproto::Message>>>,
}

impl ProgressReporter for ServerProgressReporter {
    fn done(&self) -> tokio::sync::watch::Receiver<bool> {
        let ctx = self.background_ctx.clone();
        let (tx, rx) = tokio::sync::watch::channel(ctx.err().is_some());
        spawn_progress_task(async move {
            while ctx.err().is_none() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let _ = tx.send(true);
        });
        rx
    }

    fn localize(&self, msg: &diagnostics::Message, args: Vec<ProgressArg>) -> String {
        diagnostics::localize(
            self.locale.clone(),
            Some(msg),
            String::new(),
            args.into_iter().map(|arg| arg.to_string()),
        )
    }

    fn create_work_done_progress(&self, token: String) {
        let seq = self
            .client_seq
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        let id = ts_jsonrpc::Id::new_string(format!("ts{seq}"));
        let req = lsproto::WindowWorkDoneProgressCreateInfo.new_request_message(
            Some(id),
            lsproto::WorkDoneProgressCreateParams {
                token: lsproto::IntegerOrString::from(token),
            },
        );
        self.outgoing_queue
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(req.message());
    }

    fn send_progress(&self, token: String, value: lsproto::WorkDoneProgressBeginOrReportOrEnd) {
        let notification =
            lsproto::ProgressInfo.new_notification_message(lsproto::ProgressParams {
                token: lsproto::IntegerOrString::from(token),
                value,
            });
        self.outgoing_queue
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(notification.message());
    }
}

// projectLoadingProgress manages LSP WorkDoneProgress indicators for
// long-running operations. A single persistent task processes
// start/finish events, maintains a ref-counted map of active operations,
// and sends progress messages in order.
//
// To avoid flickering on fast operations, the indicator is not shown
// until progressDelay has elapsed since the first start event. If all
// operations complete before then, no progress UI is displayed.
//
// start/finish may block if the internal buffer (64 events) is full,
// but will bail out if the server's background context is cancelled.
#[derive(Clone)]
pub struct ProjectLoadingProgress {
    reporter: Arc<dyn ProgressReporter + Send + Sync>,
    ch: tokio::sync::mpsc::Sender<ProgressEvent>,
    delay: Duration,
}

pub fn new_project_loading_progress(server: &Server, delay: Duration) -> ProjectLoadingProgress {
    new_project_loading_progress_from_reporter(
        Arc::new(ServerProgressReporter {
            background_ctx: server.background_ctx.clone().unwrap_or_default(),
            locale: server.locale.clone().unwrap_or_default(),
            client_seq: server.client_seq.clone(),
            outgoing_queue: server.outgoing_queue.clone(),
        }),
        delay,
    )
}

pub fn new_project_loading_progress_from_reporter(
    reporter: Arc<dyn ProgressReporter + Send + Sync>,
    delay: Duration,
) -> ProjectLoadingProgress {
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    let progress = ProjectLoadingProgress {
        reporter: reporter.clone(),
        ch: tx,
        delay,
    };
    spawn_progress_task(run_project_loading_progress(reporter, rx, delay));
    progress
}

impl ProjectLoadingProgress {
    fn event(
        message: &diagnostics::Message,
        args: Vec<ProgressArg>,
        finish: bool,
    ) -> ProgressEvent {
        ProgressEvent {
            message: Some(message.clone()),
            args,
            finish,
        }
    }

    pub fn start_blocking(&self, message: &diagnostics::Message, args: Vec<ProgressArg>) {
        let done = self.reporter.done();
        if *done.borrow() {
            return;
        }
        let event = Self::event(message, args, false);
        let _ = self.ch.blocking_send(event);
    }

    pub fn finish_blocking(&self, message: &diagnostics::Message, args: Vec<ProgressArg>) {
        let done = self.reporter.done();
        if *done.borrow() {
            return;
        }
        let event = Self::event(message, args, true);
        let _ = self.ch.blocking_send(event);
    }

    pub async fn start(&self, message: &diagnostics::Message, args: Vec<ProgressArg>) {
        let mut done = self.reporter.done();
        if *done.borrow() {
            return;
        }
        let event = Self::event(message, args, false);
        tokio::select! {
            result = self.ch.send(event) => {
                let _ = result;
            }
            _ = done.changed() => {
                // Server shutting down; drop the event.
            }
        }
    }

    pub async fn finish(&self, message: &diagnostics::Message, args: Vec<ProgressArg>) {
        let mut done = self.reporter.done();
        if *done.borrow() {
            return;
        }
        let event = Self::event(message, args, true);
        tokio::select! {
            result = self.ch.send(event) => {
                let _ = result;
            }
            _ = done.changed() => {
                // Server shutting down; drop the event.
            }
        }
    }
}

// run is the persistent task that processes all progress events.
// It owns all mutable state: no external synchronization needed.
async fn run_project_loading_progress(
    reporter: Arc<dyn ProgressReporter + Send + Sync>,
    mut ch: tokio::sync::mpsc::Receiver<ProgressEvent>,
    delay: Duration,
) {
    let mut loading: collections::OrderedMap<String, i32> = collections::OrderedMap::default();
    let mut token = String::new();
    let mut token_id = 0;
    let mut begun = false;
    let mut delay_task: Option<Pin<Box<tokio::time::Sleep>>> = None;
    let mut delay_fired = false;
    let mut done = reporter.done();
    if *done.borrow() {
        return;
    }

    loop {
        tokio::select! {
            biased;

            _ = done.changed() => {
                delay_task = None;
                return;
            }

            _ = async {
                if let Some(delay_task) = &mut delay_task {
                    delay_task.await;
                }
            }, if delay_task.is_some() => {
                delay_fired = true;
                if !token.is_empty() && loading.size() > 0 {
                    reporter.create_work_done_progress(token.clone());
                    let first = core::first_or_nil_seq(loading.keys().cloned());
                    begun = begin_or_report(reporter.clone(), &token, &first, begun);
                }
                delay_task = None;
            }

            maybe_event = ch.recv() => {
                let Some(ev) = maybe_event else {
                    delay_task = None;
                    return;
                };

                let message = ev.message.as_ref().expect("progress event message should be set");
                let text = reporter.localize(message, ev.args);
                if !ev.finish {
                    let count = loading.get_or_zero(&text);
                    loading.set(text.clone(), count + 1);
                    if token.is_empty() {
                        token_id += 1;
                        token = format!("tsgo-loading-{token_id}");
                        begun = false;
                        if delay.is_zero() {
                            delay_fired = true;
                            reporter.create_work_done_progress(token.clone());
                        } else {
                            delay_fired = false;
                            delay_task = Some(Box::pin(tokio::time::sleep(delay)));
                        }
                    }
                    if delay_fired {
                        begun = begin_or_report(reporter.clone(), &token, &text, begun);
                    }
                } else {
                    let count = loading.get_or_zero(&text);
                    if count <= 1 {
                        loading.delete(&text);
                    } else {
                        loading.set(text.clone(), count - 1);
                    }
                    if token.is_empty() {
                        continue;
                    }
                    if loading.size() == 0 {
                        if begun {
                            reporter.send_progress(token.clone(), lsproto::WorkDoneProgressBeginOrReportOrEnd {
                                end: Some(lsproto::WorkDoneProgressEnd::default()),
                                ..Default::default()
                            });
                        }
                        delay_task = None;
                        token.clear();
                    } else if delay_fired {
                        let first = core::first_or_nil_seq(loading.keys().cloned());
                        reporter.send_progress(token.clone(), lsproto::WorkDoneProgressBeginOrReportOrEnd {
                            report: Some(lsproto::WorkDoneProgressReport {
                                message: Some(first),
                                ..Default::default()
                            }),
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }
}

// beginOrReport sends WorkDoneProgressBegin if not yet begun, otherwise
// sends WorkDoneProgressReport. Returns true to indicate begun state.
fn begin_or_report(
    reporter: Arc<dyn ProgressReporter + Send + Sync>,
    token: &str,
    text: &str,
    begun: bool,
) -> bool {
    if !begun {
        let title = reporter.localize(&diagnostics::LOADING, Vec::new());
        reporter.send_progress(
            token.to_string(),
            lsproto::WorkDoneProgressBeginOrReportOrEnd {
                begin: Some(lsproto::WorkDoneProgressBegin {
                    title,
                    message: Some(text.to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
    } else {
        reporter.send_progress(
            token.to_string(),
            lsproto::WorkDoneProgressBeginOrReportOrEnd {
                report: Some(lsproto::WorkDoneProgressReport {
                    message: Some(text.to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
    }
    true
}
