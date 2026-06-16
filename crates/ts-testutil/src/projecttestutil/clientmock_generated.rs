pub type WatcherId = String;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Message {
    pub text: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PublishDiagnosticsParams {
    pub uri: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TelemetryEvent {
    pub name: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileSystemWatcher {
    pub glob_pattern: String,
}

#[derive(Default)]
pub struct ClientMock {
    pub is_active_func: Option<Box<dyn Fn() -> bool + Send + Sync>>,
    pub progress_finish_func: Option<Box<dyn Fn(&Message, &[String]) + Send + Sync>>,
    pub progress_start_func: Option<Box<dyn Fn(&Message, &[String]) + Send + Sync>>,
    pub publish_diagnostics_func:
        Option<Box<dyn Fn(&PublishDiagnosticsParams) -> Result<(), String> + Send + Sync>>,
    pub refresh_code_lens_func: Option<Box<dyn Fn() -> Result<(), String> + Send + Sync>>,
    pub refresh_diagnostics_func: Option<Box<dyn Fn() -> Result<(), String> + Send + Sync>>,
    pub refresh_inlay_hints_func: Option<Box<dyn Fn() -> Result<(), String> + Send + Sync>>,
    pub send_telemetry_func:
        Option<Box<dyn Fn(&TelemetryEvent) -> Result<(), String> + Send + Sync>>,
    pub unwatch_files_func: Option<Box<dyn Fn(&WatcherId) -> Result<(), String> + Send + Sync>>,
    pub watch_files_func:
        Option<Box<dyn Fn(&WatcherId, &[FileSystemWatcher]) -> Result<(), String> + Send + Sync>>,

    is_active_calls: Vec<()>,
    progress_finish_calls: Vec<(Message, Vec<String>)>,
    progress_start_calls: Vec<(Message, Vec<String>)>,
    publish_diagnostics_calls: Vec<PublishDiagnosticsParams>,
    refresh_code_lens_calls: Vec<()>,
    refresh_diagnostics_calls: Vec<()>,
    refresh_inlay_hints_calls: Vec<()>,
    send_telemetry_calls: Vec<TelemetryEvent>,
    unwatch_files_calls: Vec<WatcherId>,
    watch_files_calls: Vec<(WatcherId, Vec<FileSystemWatcher>)>,
}

impl ClientMock {
    pub fn is_active(&mut self) -> bool {
        self.is_active_calls.push(());
        self.is_active_func
            .as_ref()
            .map(|f| f())
            .unwrap_or_default()
    }

    pub fn is_active_calls(&self) -> Vec<()> {
        self.is_active_calls.clone()
    }

    pub fn progress_finish(&mut self, message: &Message, args: &[String]) {
        self.progress_finish_calls
            .push((message.clone(), args.to_vec()));
        if let Some(f) = &self.progress_finish_func {
            f(message, args);
        }
    }

    pub fn progress_finish_calls(&self) -> Vec<(Message, Vec<String>)> {
        self.progress_finish_calls.clone()
    }

    pub fn progress_start(&mut self, message: &Message, args: &[String]) {
        self.progress_start_calls
            .push((message.clone(), args.to_vec()));
        if let Some(f) = &self.progress_start_func {
            f(message, args);
        }
    }

    pub fn progress_start_calls(&self) -> Vec<(Message, Vec<String>)> {
        self.progress_start_calls.clone()
    }

    pub fn publish_diagnostics(&mut self, params: &PublishDiagnosticsParams) -> Result<(), String> {
        self.publish_diagnostics_calls.push(params.clone());
        if let Some(f) = &self.publish_diagnostics_func {
            return f(params);
        }
        Ok(())
    }

    pub fn publish_diagnostics_calls(&self) -> Vec<PublishDiagnosticsParams> {
        self.publish_diagnostics_calls.clone()
    }

    pub fn refresh_code_lens(&mut self) -> Result<(), String> {
        self.refresh_code_lens_calls.push(());
        if let Some(f) = &self.refresh_code_lens_func {
            return f();
        }
        Ok(())
    }

    pub fn refresh_code_lens_calls(&self) -> Vec<()> {
        self.refresh_code_lens_calls.clone()
    }

    pub fn refresh_diagnostics(&mut self) -> Result<(), String> {
        self.refresh_diagnostics_calls.push(());
        if let Some(f) = &self.refresh_diagnostics_func {
            return f();
        }
        Ok(())
    }

    pub fn refresh_diagnostics_calls(&self) -> Vec<()> {
        self.refresh_diagnostics_calls.clone()
    }

    pub fn refresh_inlay_hints(&mut self) -> Result<(), String> {
        self.refresh_inlay_hints_calls.push(());
        if let Some(f) = &self.refresh_inlay_hints_func {
            return f();
        }
        Ok(())
    }

    pub fn refresh_inlay_hints_calls(&self) -> Vec<()> {
        self.refresh_inlay_hints_calls.clone()
    }

    pub fn send_telemetry(&mut self, telemetry: &TelemetryEvent) -> Result<(), String> {
        self.send_telemetry_calls.push(telemetry.clone());
        if let Some(f) = &self.send_telemetry_func {
            return f(telemetry);
        }
        Ok(())
    }

    pub fn send_telemetry_calls(&self) -> Vec<TelemetryEvent> {
        self.send_telemetry_calls.clone()
    }

    pub fn unwatch_files(&mut self, id: &WatcherId) -> Result<(), String> {
        self.unwatch_files_calls.push(id.clone());
        if let Some(f) = &self.unwatch_files_func {
            return f(id);
        }
        Ok(())
    }

    pub fn unwatch_files_calls(&self) -> Vec<WatcherId> {
        self.unwatch_files_calls.clone()
    }

    pub fn watch_files(
        &mut self,
        id: &WatcherId,
        watchers: &[FileSystemWatcher],
    ) -> Result<(), String> {
        self.watch_files_calls.push((id.clone(), watchers.to_vec()));
        if let Some(f) = &self.watch_files_func {
            return f(id, watchers);
        }
        Ok(())
    }

    pub fn watch_files_calls(&self) -> Vec<(WatcherId, Vec<FileSystemWatcher>)> {
        self.watch_files_calls.clone()
    }
}

// --------------------------------------------------------------------------
// PORT STATUS
//   source:     internal/testutil/projecttestutil/clientmock_generated.go (514 lines)
//   confidence: medium
//   todos:      replace temporary diagnostics/lsproto/project aliases with final
//               crate types when the project Client trait is stable
//   notes:      preserves generated mock callbacks, default return behavior,
//               per-method call recording, and calls accessors
// --------------------------------------------------------------------------
