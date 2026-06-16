use crate::watch::WatcherId;
use std::sync::Arc;

pub type WatcherID = WatcherId;
pub type FileSystemWatcher = ts_lsproto::FileSystemWatcher;
pub type PublishDiagnosticsParams = ts_lsproto::PublishDiagnosticsParams;
pub type DiagnosticsMessage = ts_diagnostics::Message;
pub type Context = ts_core::Context;
pub type TelemetryEvent = ts_lsproto::TelemetryEvent;

#[derive(Clone)]
pub struct ClientHandle {
    client: Arc<dyn Client>,
}

pub trait Client: Send + Sync {
    fn watch_files(
        &self,
        ctx: &Context,
        id: WatcherID,
        watchers: Vec<FileSystemWatcher>,
    ) -> Result<(), String>;
    fn unwatch_files(&self, ctx: &Context, id: WatcherID) -> Result<(), String>;
    fn refresh_diagnostics(&self, ctx: &Context) -> Result<(), String>;
    fn publish_diagnostics(
        &self,
        ctx: &Context,
        params: PublishDiagnosticsParams,
    ) -> Result<(), String>;
    fn refresh_inlay_hints(&self, ctx: &Context) -> Result<(), String>;
    fn refresh_code_lens(&self, ctx: &Context) -> Result<(), String>;
    fn progress_start(&self, message: &DiagnosticsMessage, args: &[String]);
    fn progress_finish(&self, message: &DiagnosticsMessage, args: &[String]);
    fn send_telemetry(&self, ctx: &Context, telemetry: TelemetryEvent) -> Result<(), String>;
    fn is_active(&self) -> bool;
}

pub trait ClientArcExt {
    fn clone_handle(&self) -> ClientHandle;
}

impl ClientArcExt for Arc<dyn Client> {
    fn clone_handle(&self) -> ClientHandle {
        ClientHandle {
            client: Arc::clone(self),
        }
    }
}

impl Client for ClientHandle {
    fn watch_files(
        &self,
        ctx: &Context,
        id: WatcherID,
        watchers: Vec<FileSystemWatcher>,
    ) -> Result<(), String> {
        self.client.watch_files(ctx, id, watchers)
    }

    fn unwatch_files(&self, ctx: &Context, id: WatcherID) -> Result<(), String> {
        self.client.unwatch_files(ctx, id)
    }

    fn refresh_diagnostics(&self, ctx: &Context) -> Result<(), String> {
        self.client.refresh_diagnostics(ctx)
    }

    fn publish_diagnostics(
        &self,
        ctx: &Context,
        params: PublishDiagnosticsParams,
    ) -> Result<(), String> {
        self.client.publish_diagnostics(ctx, params)
    }

    fn refresh_inlay_hints(&self, ctx: &Context) -> Result<(), String> {
        self.client.refresh_inlay_hints(ctx)
    }

    fn refresh_code_lens(&self, ctx: &Context) -> Result<(), String> {
        self.client.refresh_code_lens(ctx)
    }

    fn progress_start(&self, message: &DiagnosticsMessage, args: &[String]) {
        self.client.progress_start(message, args)
    }

    fn progress_finish(&self, message: &DiagnosticsMessage, args: &[String]) {
        self.client.progress_finish(message, args)
    }

    fn send_telemetry(&self, ctx: &Context, telemetry: TelemetryEvent) -> Result<(), String> {
        self.client.send_telemetry(ctx, telemetry)
    }

    fn is_active(&self) -> bool {
        self.client.is_active()
    }
}
