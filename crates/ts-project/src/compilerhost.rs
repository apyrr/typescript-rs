use std::sync::{Arc, Mutex};

use ts_ast as ast;
use ts_collections as collections;
use ts_compiler as compiler;
use ts_diagnostics as diagnostics;
use ts_locale as locale;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;
use ts_vfs as vfs;

use crate::logging::{LogTree, Logger};
use crate::{
    ConfigFileRegistry, ConfigFileRegistryBuilder, ParseCache, ProjectCollectionBuilder,
    SessionOptions, SnapshotFs, SourceFs, new_parse_cache_key, new_source_fs,
};

pub struct CompilerHost {
    pub config_file_path: tspath::Path,
    pub current_directory: String,
    pub session_options: SessionOptions,
    pub source_fs: SourceFs,
    state: Mutex<CompilerHostState>,
}

struct CompilerHostLive {
    config_file_registry_builder: ConfigFileRegistryBuilder,
    parse_cache: ParseCache,
    project_config_file_path: tspath::Path,
    logger: Arc<LogTree>,
}

enum CompilerHostState {
    Live(CompilerHostLive),
    Frozen(ConfigFileRegistry),
}

impl Clone for CompilerHost {
    fn clone(&self) -> Self {
        let state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        let state = match &*state {
            CompilerHostState::Live(live) => CompilerHostState::Live(CompilerHostLive {
                config_file_registry_builder: live.config_file_registry_builder.clone(),
                parse_cache: live.parse_cache.clone(),
                project_config_file_path: live.project_config_file_path.clone(),
                logger: live.logger.clone(),
            }),
            CompilerHostState::Frozen(registry) => CompilerHostState::Frozen(registry.clone()),
        };
        Self {
            config_file_path: self.config_file_path.clone(),
            current_directory: self.current_directory.clone(),
            session_options: self.session_options.clone(),
            source_fs: self.source_fs.clone(),
            state: Mutex::new(state),
        }
    }
}

pub fn new_compiler_host(
    current_directory: String,
    config_file_path: tspath::Path,
    builder: &ProjectCollectionBuilder,
    logger: Arc<LogTree>,
) -> CompilerHost {
    let to_path = builder.to_path.clone();
    CompilerHost {
        config_file_path: config_file_path.clone(),
        current_directory,
        session_options: builder.session_options.clone(),
        source_fs: new_source_fs(
            true,
            builder.fs.clone(),
            Arc::new(move |file_name| to_path(file_name.to_string())),
        ),
        state: Mutex::new(CompilerHostState::Live(CompilerHostLive {
            config_file_registry_builder: builder.config_file_registry_builder.clone(),
            parse_cache: builder.parse_cache.clone(),
            project_config_file_path: config_file_path,
            logger,
        })),
    }
}

pub fn new_compiler_host_handle(host: &Arc<CompilerHost>) -> Arc<dyn compiler::CompilerHost> {
    host.clone()
}

impl CompilerHost {
    pub fn freeze(&self, snapshot_fs: SnapshotFs, config_file_registry: ConfigFileRegistry) {
        let mut state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        if !matches!(&*state, CompilerHostState::Live(_)) {
            panic!("freeze can only be called once");
        }
        self.source_fs.set_source(snapshot_fs);
        self.source_fs.disable_tracking();
        *state = CompilerHostState::Frozen(config_file_registry);
    }

    pub fn ensure_alive(&self) {
        if !matches!(
            &*self.state.lock().unwrap_or_else(|err| err.into_inner()),
            CompilerHostState::Live(_)
        ) {
            panic!("method must not be called after snapshot initialization");
        }
    }

    pub fn set_seen_files(&self, seen_files: Option<collections::SyncSet<tspath::Path>>) {
        self.source_fs.set_seen_files(seen_files);
    }

    pub fn parse_cache(&self) -> ParseCache {
        let state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        match &*state {
            CompilerHostState::Live(live) => live.parse_cache.clone(),
            CompilerHostState::Frozen(_) => {
                panic!("method must not be called after snapshot initialization")
            }
        }
    }
}

impl compiler::CompilerHost for CompilerHost {
    fn default_library_path(&self) -> String {
        self.session_options.default_library_path.clone()
    }

    fn fs(&self) -> &dyn vfs::Fs {
        &self.source_fs
    }

    fn get_current_directory(&self) -> String {
        self.current_directory.clone()
    }

    fn get_resolved_project_reference(
        &self,
        file_name: &str,
        path: tspath::Path,
    ) -> Option<tsoptions::ParsedCommandLine> {
        let state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        match &*state {
            CompilerHostState::Frozen(registry) => registry.get_config(path),
            CompilerHostState::Live(live) => {
                self.source_fs.track(file_name);
                live.config_file_registry_builder
                    .acquire_config_for_project(
                        file_name,
                        path,
                        &live.project_config_file_path,
                        live.logger.as_ref(),
                    )
            }
        }
    }

    fn get_parsed_source_file(
        &self,
        opts: ast::SourceFileParseOptions,
    ) -> Option<ast::ParsedSourceFile> {
        self.ensure_alive();
        let fh = self
            .source_fs
            .get_file_by_path(&opts.file_name, &opts.path)?;
        let key = new_parse_cache_key(opts, fh.hash(), fh.kind());
        Some(self.parse_cache().acquire(key, fh))
    }

    fn trace(&self, msg: &'static diagnostics::Message, args: &compiler::DiagnosticArgs) {
        let state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        let CompilerHostState::Live(live) = &*state else {
            panic!("method must not be called after snapshot initialization");
        };
        live.logger
            .logf(msg.localize(locale::DEFAULT, args.to_vec()));
    }
}
