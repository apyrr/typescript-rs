use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime};

use serde_json::{Map, Number, Value};
use ts_ast as ast;
use ts_compiler as compiler;
use ts_diagnostics as diagnostics;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;
use ts_vfs::cachedvfs;

pub use ts_collections::SyncMap;

use crate::incremental;
use crate::tsc;

use super::buildtask::BuildTaskHandle;
use super::compiler_host::{DiagnosticArgs, DiagnosticsMessage};
use super::parse_cache::ParseCache;

pub struct Host<H>
where
    H: compiler::CompilerHost,
{
    pub sys: tsc::System,
    pub command: tsoptions::ParsedBuildCommandLine,
    pub compare_paths_options: tspath::ComparePathsOptions,
    pub tasks: RwLock<SyncMap<tspath::Path, BuildTaskHandle>>,
    pub host: H,
    pub cached_fs: Arc<cachedvfs::CachedFs>,

    // Caches that last only for build cycle and then cleared out
    pub extended_config_cache: Mutex<tsc::ExtendedConfigCache>,
    pub(crate) source_files: ParseCache<ast::SourceFileParseOptions, Option<ast::ParsedSourceFile>>,
    pub config_times: Mutex<SyncMap<tspath::Path, Duration>>,

    // caches that stay as long as they are needed
    pub(crate) resolved_references: ParseCache<tspath::Path, Option<tsoptions::ParsedCommandLine>>,
    pub m_times: RwLock<Arc<SyncMap<tspath::Path, SystemTime>>>,
}

impl<H> Host<H>
where
    H: compiler::CompilerHost,
{
    pub fn new(
        sys: tsc::System,
        command: tsoptions::ParsedBuildCommandLine,
        compare_paths_options: tspath::ComparePathsOptions,
        tasks: SyncMap<tspath::Path, BuildTaskHandle>,
        host: H,
        cached_fs: Arc<cachedvfs::CachedFs>,
        m_times: Arc<SyncMap<tspath::Path, SystemTime>>,
    ) -> Self {
        Self {
            sys,
            command,
            compare_paths_options,
            tasks: RwLock::new(tasks),
            host,
            cached_fs,
            extended_config_cache: Mutex::new(tsc::ExtendedConfigCache::default()),
            source_files: ParseCache::default(),
            config_times: Mutex::new(SyncMap::default()),
            resolved_references: ParseCache::default(),
            m_times: RwLock::new(m_times),
        }
    }

    pub fn fs(&self) -> &dyn ts_vfs::Fs {
        self.host.fs()
    }

    pub fn clear_fs_cache(&self) {
        self.cached_fs.clear_cache();
    }

    pub fn default_library_path(&self) -> String {
        self.host.default_library_path().to_owned()
    }

    pub fn to_path(&self, file_name: &str) -> tspath::Path {
        tspath::to_path(
            file_name,
            &self.compare_paths_options.current_directory,
            self.compare_paths_options.use_case_sensitive_file_names,
        )
    }

    pub fn set_tasks(&self, tasks: SyncMap<tspath::Path, BuildTaskHandle>) {
        *self.tasks.write().unwrap_or_else(|err| err.into_inner()) = tasks;
    }

    pub fn get_task(&self, path: tspath::Path) -> BuildTaskHandle {
        let tasks = self.tasks.read().unwrap_or_else(|err| err.into_inner());
        let (task, ok) = tasks.load(&path);
        if !ok {
            panic!("No build task found for {path}");
        }
        task.unwrap()
    }

    pub fn get_current_directory(&self) -> String {
        self.host.get_current_directory()
    }

    pub fn trace(&self, _msg: &'static diagnostics::Message, _args: &DiagnosticArgs) {
        panic!("build.Orchestrator.host does not support tracing, use a different host for tracing")
    }

    pub fn get_source_file(&self, opts: ast::SourceFileParseOptions) -> Option<ast::SourceFile> {
        self.get_parsed_source_file(opts)
            .map(ast::ParsedSourceFile::into_source_file)
    }

    fn get_parsed_source_file(
        &self,
        opts: ast::SourceFileParseOptions,
    ) -> Option<ast::ParsedSourceFile> {
        if tspath::is_declaration_file_name(&opts.file_name)
            || tspath::file_extension_is(&opts.file_name, tspath::EXTENSION_JSON)
        {
            // Cache dts and json files as they will be reused
            return self.source_files.load_or_store(
                opts,
                |opts| self.host.get_parsed_source_file(opts),
                false,
            );
        }
        self.host.get_parsed_source_file(opts)
    }

    pub fn get_resolved_project_reference(
        &self,
        file_name: String,
        path: tspath::Path,
    ) -> Option<tsoptions::ParsedCommandLine> {
        self.resolved_references.load_or_store(
            path,
            |path| {
                let config_start = self.sys.now();
                let command_line_raw = build_command_line_raw(&self.command.raw);
                let extended_config_cache = self
                    .extended_config_cache
                    .lock()
                    .unwrap_or_else(|err| err.into_inner());
                let command_line = tsoptions::get_parsed_command_line_of_config_file_path(
                    &file_name,
                    path.clone(),
                    Some(&self.command.compiler_options),
                    command_line_raw.as_ref(),
                    self,
                    Some(&*extended_config_cache),
                )
                .0;
                let config_time = self
                    .sys
                    .now()
                    .duration_since(config_start)
                    .unwrap_or_default();
                self.config_times
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .store(path, Some(config_time));
                command_line
            },
            true,
        )
    }

    pub fn read_build_info(
        &self,
        config: &tsoptions::ParsedCommandLine,
    ) -> Option<incremental::BuildInfo> {
        let config_path = self.to_path(&config.config_name());
        let task = self.get_task(config_path);
        let (build_info, _) = task
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .load_or_store_build_info(self, &config.get_build_info_file_name());
        build_info
    }

    pub fn get_m_time(&self, file: &str) -> SystemTime {
        self.load_or_store_m_time(file, None, true)
    }

    pub fn set_m_time(&self, file: &str, m_time: SystemTime) -> Result<(), String> {
        self.fs()
            .chtimes(file, SystemTime::UNIX_EPOCH, m_time)
            .map_err(|err| err.to_string())
    }

    pub fn load_or_store_m_time(
        &self,
        file: &str,
        old_cache: Option<&SyncMap<tspath::Path, SystemTime>>,
        store: bool,
    ) -> SystemTime {
        let path = self.to_path(file);
        let cache = self.m_times();
        let (existing, loaded) = cache.load(&path);
        if loaded {
            return existing.unwrap_or(SystemTime::UNIX_EPOCH);
        }
        let mut found = false;
        let mut m_time = SystemTime::UNIX_EPOCH;
        if let Some(old_cache) = old_cache {
            let (old_m_time, old_found) = old_cache.load(&path);
            if old_found {
                m_time = old_m_time.unwrap_or(SystemTime::UNIX_EPOCH);
                found = true;
            }
        }
        if !found {
            m_time = incremental::get_m_time(&self.host, file);
        }
        if store {
            let cache = self.m_times();
            let (stored_m_time, _) = cache.load_or_store(path, Some(m_time));
            m_time = stored_m_time.unwrap_or(SystemTime::UNIX_EPOCH);
        }
        m_time
    }

    pub fn store_m_time(&self, file: &str, m_time: SystemTime) {
        let path = self.to_path(file);
        self.m_times().store(path, Some(m_time));
    }

    pub fn store_m_time_from_old_cache(
        &self,
        file: &str,
        old_cache: &SyncMap<tspath::Path, SystemTime>,
    ) {
        let path = self.to_path(file);
        let (m_time, found) = old_cache.load(&path);
        if found {
            self.m_times().store(path, m_time);
        }
    }

    pub fn m_times(&self) -> Arc<SyncMap<tspath::Path, SystemTime>> {
        self.m_times
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }

    pub fn replace_m_times(&self, m_times: Arc<SyncMap<tspath::Path, SystemTime>>) {
        *self.m_times.write().unwrap_or_else(|err| err.into_inner()) = m_times;
    }

    pub fn replace_config_times(&self, config_times: SyncMap<tspath::Path, Duration>) {
        *self
            .config_times
            .lock()
            .unwrap_or_else(|err| err.into_inner()) = config_times;
    }

    pub fn reset_extended_config_cache(&self) {
        *self
            .extended_config_cache
            .lock()
            .unwrap_or_else(|err| err.into_inner()) = tsc::ExtendedConfigCache::default();
    }

    pub fn config_time(&self, path: &tspath::Path) -> Duration {
        self.config_times
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .load(path)
            .0
            .unwrap_or_default()
    }

    pub fn store_config_time(&self, path: tspath::Path, config_time: Duration) {
        self.config_times
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .store(path, Some(config_time));
    }
}

fn build_command_line_raw(args: &[String]) -> Option<Value> {
    let mut raw = Map::new();
    let build_map = tsoptions::build_name_map(&tsoptions::build_opts());
    let compiler_map = tsoptions::compiler_name_map(tsoptions::options_declarations());
    let watch_map = tsoptions::watch_name_map(&tsoptions::options_for_watch());
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        index += 1;
        if !arg.starts_with('-') {
            continue;
        }
        let input_name = arg.trim_start_matches('-');
        let option = build_map
            .get_option_declaration_from_name(input_name, true)
            .or_else(|| compiler_map.get_option_declaration_from_name(input_name, true))
            .or_else(|| watch_map.get_option_declaration_from_name(input_name, true));
        let Some(option) = option else {
            continue;
        };
        let next = args.get(index);
        if next.is_some_and(|value| value == "null") {
            raw.insert(option.name.clone(), Value::Null);
            index += 1;
            continue;
        }
        match option.kind {
            Some(tsoptions::CommandLineOptionKind::Boolean) => {
                let value = next.is_none_or(|value| value != "false");
                if next.is_some_and(|value| value == "true" || value == "false") {
                    index += 1;
                }
                raw.insert(option.name.clone(), Value::Bool(value));
            }
            Some(tsoptions::CommandLineOptionKind::Number) => {
                if let Some(value) = next {
                    if !value.starts_with('-') {
                        if let Ok(value) = value.parse::<i64>() {
                            raw.insert(option.name.clone(), Value::Number(Number::from(value)));
                        }
                        index += 1;
                    }
                }
            }
            Some(tsoptions::CommandLineOptionKind::List)
            | Some(tsoptions::CommandLineOptionKind::ListOrElement) => {
                if let Some(value) = next {
                    if value.starts_with('-') {
                        raw.insert(option.name.clone(), Value::Array(Vec::new()));
                    } else {
                        raw.insert(
                            option.name.clone(),
                            Value::Array(
                                value
                                    .split(',')
                                    .filter(|part| !part.is_empty())
                                    .map(|part| Value::String(part.trim().to_owned()))
                                    .collect(),
                            ),
                        );
                        index += 1;
                    }
                } else {
                    raw.insert(option.name.clone(), Value::Array(Vec::new()));
                }
            }
            _ => {
                if let Some(value) = next {
                    if !value.starts_with('-') {
                        raw.insert(option.name.clone(), Value::String(value.clone()));
                        index += 1;
                    }
                }
            }
        }
    }
    (!raw.is_empty()).then(|| {
        let mut wrapped = Map::new();
        wrapped.insert("compilerOptions".to_owned(), Value::Object(raw));
        Value::Object(wrapped)
    })
}

impl<H> incremental::BuildInfoReader for Host<H>
where
    H: compiler::CompilerHost,
{
    fn read_build_info(
        &self,
        config: &tsoptions::ParsedCommandLine,
    ) -> Option<incremental::BuildInfo> {
        Host::read_build_info(self, config)
    }
}

impl<H> incremental::Host for Host<H>
where
    H: compiler::CompilerHost,
{
    fn get_m_time(&self, file_name: &str) -> SystemTime {
        Host::get_m_time(self, file_name)
    }

    fn set_m_time(&self, file_name: &str, m_time: SystemTime) -> Result<(), String> {
        Host::set_m_time(self, file_name, m_time)
    }
}

impl<H> compiler::CompilerHost for Host<H>
where
    H: compiler::CompilerHost,
{
    fn fs(&self) -> &dyn ts_vfs::Fs {
        self.fs()
    }

    fn default_library_path(&self) -> String {
        self.default_library_path()
    }

    fn get_current_directory(&self) -> String {
        self.get_current_directory()
    }

    fn trace(&self, msg: &'static DiagnosticsMessage, args: &DiagnosticArgs) {
        self.trace(msg, args)
    }

    fn get_parsed_source_file(
        &self,
        opts: ast::SourceFileParseOptions,
    ) -> Option<ast::ParsedSourceFile> {
        Host::get_parsed_source_file(self, opts)
    }

    fn get_resolved_project_reference(
        &self,
        file_name: &str,
        path: tspath::Path,
    ) -> Option<tsoptions::ParsedCommandLine> {
        self.get_resolved_project_reference(file_name.to_owned(), path)
            .map(Into::into)
    }
}

impl<H> tsoptions::ParseConfigHost for Host<H>
where
    H: compiler::CompilerHost,
{
    fn fs(&self) -> &dyn ts_vfs::Fs {
        self.fs()
    }

    fn get_current_directory(&self) -> String {
        self.get_current_directory()
    }
}
