use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use ts_collections as collections;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_locale as locale;
use ts_testutil::{fsbaselineutil, harnessutil, stringtestutil};
use ts_tspath as tspath;
use ts_vfs as vfs;
use ts_vfs::Fs;
use ts_vfs::vfstest::{self, IntoMapFile};

use crate::{incremental, tsc};

use super::fs::TestFs;

pub type FileMap = HashMap<String, vfstest::MapFile>;

pub const TSC_LIB_PATH: &str = "/home/src/tslibs/TS/Lib";

pub fn tsc_default_lib_content() -> String {
    stringtestutil::dedent(
        r#"
/// <reference no-default-lib="true"/>
interface Boolean {}
interface Function {}
interface CallableFunction {}
interface NewableFunction {}
interface IArguments {}
interface Number { toExponential: any; }
interface Object {}
interface RegExp {}
interface String { charAt: any; }
interface Array<T> { length: number; [n: number]: T; }
interface ReadonlyArray<T> {}
interface SymbolConstructor {
    (desc?: string | number): symbol;
    for(name: string): symbol;
    readonly toStringTag: symbol;
}
declare var Symbol: SymbolConstructor;
interface Symbol {
    readonly [Symbol.toStringTag]: string;
}
declare const console: { log(msg: any): void; };
"#,
    )
}

pub fn get_test_lib_path_for(lib_name: &str) -> String {
    let lib_map = ts_tsoptions::lib_map();
    let lib_file = if let Some(lib_file) = lib_map.get(lib_name) {
        lib_file.to_string()
    } else {
        format!("lib.{lib_name}.d.ts")
    };
    format!("{TSC_LIB_PATH}/{lib_file}")
}

pub struct TestClock {
    pub start: SystemTime,
    pub now: std::sync::Mutex<Option<SystemTime>>,
}

impl vfstest::Clock for TestClock {
    fn now(&self) -> SystemTime {
        self.now()
    }

    fn since_start(&self) -> Duration {
        self.since_start()
    }
}

impl TestClock {
    pub fn now(&self) -> SystemTime {
        let mut now = self.now.lock().unwrap_or_else(|err| err.into_inner());
        let current = now.unwrap_or(self.start) + Duration::from_secs(1);
        *now = Some(current);
        current
    }

    pub fn since_start(&self) -> Duration {
        self.now()
            .duration_since(self.start)
            .unwrap_or_else(|_| Duration::default())
    }
}

pub fn new_tsc_system(files: FileMap, use_case_sensitive_file_names: bool, cwd: String) -> TestSys {
    let clock = Arc::new(TestClock {
        start: SystemTime::now(),
        now: Mutex::new(None),
    });
    let fs = TestFs {
        fs: TestMapFs(vfstest::from_map_with_clock(
            files,
            use_case_sensitive_file_names,
            clock.clone(),
        )),
        default_libs: Some(collections::SyncSet::new()),
        written_files: collections::SyncSet::new(),
    };
    TestSys {
        current_write: SharedWriter::default(),
        program_baselines: Arc::new(Mutex::new(String::new())),
        program_include_baselines: Arc::new(Mutex::new(String::new())),
        tracer: Arc::new(Mutex::new(harnessutil::TracerForBaselining::default())),
        fs: Arc::new(fs),
        fs_differ: Arc::new(Mutex::new(TestFsDiffer::default())),
        for_incremental_correctness: false,
        default_library_path: String::new(),
        cwd,
        env: HashMap::new(),
        clock,
    }
}

pub fn get_file_map_with_build(mut files: FileMap, command_line_args: Vec<String>) -> FileMap {
    let sys = new_test_sys(
        &super::runner::TscInput {
            files: files.clone(),
            sub_scenario: String::new(),
            command_line_args: Vec::new(),
            cwd: String::new(),
            edits: Vec::new(),
            env: HashMap::new(),
            ignore_case: false,
            windows_style_root: String::new(),
        },
        false,
    );
    crate::command_line(
        sys.clone_system(),
        command_line_args,
        Some(sys.clone_testing()),
    );
    sys.fs.written_files.range(|key| {
        let (text, ok) = sys.fs_from_file_map().read_file(key);
        if ok {
            let mod_time = sys
                .fs_from_file_map()
                .get_mod_time(key)
                .unwrap_or_else(|| sys.now());
            files.insert(key.clone(), text.into_map_file(mod_time));
        }
        true
    });
    files
}

pub fn new_test_sys(
    tsc_input: &super::runner::TscInput,
    for_incremental_correctness: bool,
) -> TestSys {
    let mut cwd = tsc_input.cwd.clone();
    if cwd.is_empty() {
        cwd = "/home/src/workspaces/project".to_owned();
    }
    let mut lib_path = TSC_LIB_PATH.to_owned();
    if !tsc_input.windows_style_root.is_empty() {
        lib_path = format!("{}{}", tsc_input.windows_style_root, &lib_path[1..]);
    }
    let mut sys = new_tsc_system(tsc_input.files.clone(), !tsc_input.ignore_case, cwd.clone());
    sys.default_library_path = lib_path;
    sys.tracer = Arc::new(Mutex::new(harnessutil::TracerForBaselining::new(
        !tsc_input.ignore_case,
        &cwd,
    )));
    sys.env = tsc_input.env.clone();
    sys.for_incremental_correctness = for_incremental_correctness;
    *sys.fs_differ.lock().unwrap_or_else(|err| err.into_inner()) = TestFsDiffer::default();

    sys.ensure_lib_path_exists("lib.d.ts");
    for lib_file in ts_tsoptions::target_to_lib_map().values() {
        sys.ensure_lib_path_exists(lib_file);
    }
    for lib_file in ts_tsoptions::lib_files_set() {
        sys.ensure_lib_path_exists(&lib_file);
    }
    sys
}

#[derive(Clone)]
pub struct TestSys {
    pub current_write: SharedWriter,
    pub program_baselines: Arc<Mutex<String>>,
    pub program_include_baselines: Arc<Mutex<String>>,
    pub tracer: Arc<Mutex<harnessutil::TracerForBaselining>>,
    pub fs: Arc<TestFs<TestMapFs>>,
    pub fs_differ: Arc<Mutex<TestFsDiffer>>,
    pub for_incremental_correctness: bool,
    pub default_library_path: String,
    pub cwd: String,
    pub env: HashMap<String, String>,
    pub clock: Arc<TestClock>,
}

#[derive(Clone)]
pub struct TestMapFs(pub vfstest::MapFs);

impl TestMapFs {
    pub fn get_mod_time(&self, path: &str) -> Option<SystemTime> {
        self.0.get_mod_time(path)
    }

    pub fn entries(&self) -> BTreeMap<String, fsbaselineutil::MapFsEntry> {
        self.0
            .entries()
            .into_iter()
            .map(|(path, file)| {
                let symlink_target = if file.mode.is_symlink() {
                    Some(String::from_utf8(file.data.to_vec()).unwrap_or_default())
                } else {
                    None
                };
                (
                    path,
                    fsbaselineutil::MapFsEntry {
                        data: file.data.to_vec(),
                        mtime: Some(file.mod_time),
                        is_regular: file.mode.is_file(),
                        symlink_target,
                    },
                )
            })
            .collect()
    }

    pub fn get_file_info(&self, path: &str) -> Option<fsbaselineutil::MapFsEntry> {
        self.0
            .get_file_info(path)
            .map(|file| fsbaselineutil::MapFsEntry {
                symlink_target: if file.mode.is_symlink() {
                    Some(String::from_utf8(file.data.to_vec()).unwrap_or_default())
                } else {
                    None
                },
                data: file.data.to_vec(),
                mtime: Some(file.mod_time),
                is_regular: file.mode.is_file(),
            })
    }
}

impl vfs::Fs for TestMapFs {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.0.use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        self.0.file_exists(path)
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        self.0.read_file(path)
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.0.write_file(path, data)
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.0.append_file(path, data)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        self.0.remove(path)
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        self.0.chtimes(path, atime, mtime)
    }

    fn directory_exists(&self, path: &str) -> bool {
        self.0.directory_exists(path)
    }

    fn get_accessible_entries(&self, path: &str) -> vfs::Entries {
        self.0.get_accessible_entries(path)
    }

    fn stat(&self, path: &str) -> io::Result<vfs::FileInfo> {
        self.0.stat(path)
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut vfs::WalkDirFunc<'_>) -> io::Result<()> {
        self.0.walk_dir(root, walk_fn)
    }

    fn realpath(&self, path: &str) -> String {
        self.0.realpath(path)
    }
}

#[derive(Clone, Default)]
pub struct SharedWriter {
    text: Arc<Mutex<String>>,
}

impl SharedWriter {
    pub fn string(&self) -> String {
        self.text
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }

    pub fn reset(&self) {
        self.text
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clear();
    }
}

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let s = String::from_utf8_lossy(buf);
        self.text
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push_str(&s);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct TestFsDiffer {
    pub serialized_diff: Option<fsbaselineutil::Snapshot>,
}

impl TestSys {
    pub fn clone_system(&self) -> tsc::System {
        Box::new(self.clone())
    }

    pub fn clone_testing(&self) -> tsc::CommandLineTesting {
        Box::new(self.clone())
    }

    pub fn now(&self) -> SystemTime {
        self.clock.now()
    }

    pub fn since_start(&self) -> Duration {
        self.clock.since_start()
    }

    pub fn fs(&self) -> &TestFs<TestMapFs> {
        &self.fs
    }

    pub fn fs_from_file_map(&self) -> &TestMapFs {
        &self.fs.fs
    }

    pub fn ensure_lib_path_exists(&mut self, path: &str) {
        let path = format!("{}/{path}", self.default_library_path);
        let (_, ok) = self.fs_from_file_map().read_file(&path);
        if !ok {
            if let Some(default_libs) = &self.fs.default_libs {
                default_libs.add(path.clone());
            }
            self.fs_from_file_map()
                .0
                .mkdir_all(&ts_tspath::get_directory_path(&path));
            if let Err(err) = self
                .fs_from_file_map()
                .write_file(&path, &tsc_default_lib_content())
            {
                panic!("Failed to write default library file: {err}");
            }
        }
    }

    pub fn default_library_path(&self) -> String {
        self.default_library_path.clone()
    }

    pub fn get_current_directory(&self) -> String {
        self.cwd.clone()
    }

    pub fn write_output_is_tty(&self) -> bool {
        true
    }

    pub fn get_width_of_terminal(&self) -> i32 {
        if let Some(width_str) = self.get_environment_variable("TS_TEST_TERMINAL_WIDTH") {
            return core::must(width_str.parse::<i32>());
        }
        0
    }

    pub fn get_environment_variable(&self, name: &str) -> Option<String> {
        self.env.get(name).cloned()
    }

    pub fn baseline_programs(&mut self, baseline: &mut String, header: &str) -> String {
        let mut program_baselines = self
            .program_baselines
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        baseline.push_str(&program_baselines);
        program_baselines.clear();

        let mut result = String::new();
        let mut program_include_baselines = self
            .program_include_baselines
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        if !program_include_baselines.is_empty() {
            result.push_str(&format!(
                "\n\n{header}\n!!! Include reasons expectations don't match pls review!!!\n"
            ));
            result.push_str(&program_include_baselines);
            program_include_baselines.clear();
            baseline.push_str(&result);
        }
        result
    }

    pub fn serialize_state(&mut self, baseline: &mut String) {
        self.baseline_output(baseline);
        self.baseline_fs_with_diff(baseline);
        // Watch-related state serialization is not part of this harness surface yet.
    }

    pub fn baseline_output(&self, baseline: &mut String) {
        baseline.push_str("\nOutput::\n");
        baseline.push_str(&self.get_output(false));
    }

    pub fn get_output(&self, for_comparing: bool) -> String {
        let lines = self
            .current_write
            .string()
            .split('\n')
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut transformer = OutputSanitizer {
            for_comparing,
            output_lines: Vec::with_capacity(lines.len()),
            lines,
            index: 0,
        };
        transformer.transform_lines()
    }

    pub fn clear_output(&mut self) {
        self.current_write.reset();
        self.tracer
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .reset();
    }

    pub fn baseline_fs_with_diff(&self, baseline: &mut String) {
        let mut fs_differ = self.fs_differ.lock().unwrap_or_else(|err| err.into_inner());
        fs_differ.baseline_fs_with_diff(self, baseline);
    }

    pub fn write_file_no_error(&self, path: &str, content: &str) {
        if let Err(err) = self.fs_from_file_map().write_file(path, content) {
            panic!("{err}");
        }
    }

    pub fn remove_no_error(&self, path: &str) {
        if let Err(err) = self.fs_from_file_map().remove(path) {
            panic!("{err}");
        }
    }

    pub fn read_file_no_error(&self, path: &str) -> String {
        let (content, ok) = self.fs_from_file_map().read_file(path);
        if !ok {
            panic!("File not found: {path}");
        }
        content
    }

    pub fn rename_file_no_error(&self, old_path: &str, new_path: &str) {
        self.write_file_no_error(new_path, &self.read_file_no_error(old_path));
        self.remove_no_error(old_path);
    }

    pub fn replace_file_text(&self, path: &str, old_text: &str, new_text: &str) {
        let content = self.read_file_no_error(path);
        self.write_file_no_error(path, &content.replacen(old_text, new_text, 1));
    }

    pub fn replace_file_text_all(&self, path: &str, old_text: &str, new_text: &str) {
        let content = self.read_file_no_error(path);
        self.write_file_no_error(path, &content.replace(old_text, new_text));
    }

    pub fn append_file(&self, path: &str, text: &str) {
        let content = self.read_file_no_error(path);
        self.write_file_no_error(path, &(content + text));
    }

    pub fn prepend_file(&self, path: &str, text: &str) {
        let content = self.read_file_no_error(path);
        self.write_file_no_error(path, &(text.to_owned() + &content));
    }
}

impl tsc::SystemInterface for TestSys {
    fn writer(&mut self) -> &mut dyn Write {
        &mut self.current_write
    }

    fn fs(&self) -> &dyn tsc::FileSystem {
        self.fs()
    }

    fn default_library_path(&self) -> String {
        self.default_library_path()
    }

    fn get_current_directory(&self) -> String {
        self.get_current_directory()
    }

    fn write_output_is_tty(&self) -> bool {
        self.write_output_is_tty()
    }

    fn get_width_of_terminal(&self) -> i32 {
        self.get_width_of_terminal()
    }

    fn get_environment_variable(&self, name: &str) -> String {
        self.get_environment_variable(name).unwrap_or_default()
    }

    fn now(&self) -> SystemTime {
        self.now()
    }

    fn since_start(&self) -> Duration {
        self.since_start()
    }
}

impl tsc::CommandLineTestingInterface for TestSys {
    fn on_emitted_files(&self, result: &tsc::EmitResult, m_times_cache: Option<&tsc::MTimesCache>) {
        for file in &result.emitted_files {
            let mod_time = self.fs_from_file_map().get_mod_time(file);
            let reverted = mod_time.is_some_and(|mod_time| {
                self.fs_differ
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .serialized_diff
                    .as_ref()
                    .and_then(|diff| diff.snap.get(file))
                    .is_some_and(|diff| diff.mtime == Some(mod_time))
            });
            if reverted {
                continue;
            }

            let now = self.now();
            if let Err(err) = self
                .fs_from_file_map()
                .chtimes(file, SystemTime::UNIX_EPOCH, now)
            {
                panic!("Failed to change time for emitted file: {file}: {err}");
            }
            if let Some(m_times_cache) = m_times_cache {
                let path = tspath::to_path(
                    file,
                    &self.get_current_directory(),
                    self.fs().use_case_sensitive_file_names(),
                );
                if m_times_cache.load(&path).1 {
                    m_times_cache.store(path, Some(now));
                }
            }
        }
    }

    fn on_list_files_start(&self, w: &mut dyn Write) {
        writeln!(w, "{LIST_FILE_START}").unwrap();
    }

    fn on_list_files_end(&self, w: &mut dyn Write) {
        writeln!(w, "{LIST_FILE_END}").unwrap();
    }

    fn on_statistics_start(&self, w: &mut dyn Write) {
        writeln!(w, "{STATISTICS_START}").unwrap();
    }

    fn on_statistics_end(&self, w: &mut dyn Write) {
        writeln!(w, "{STATISTICS_END}").unwrap();
    }

    fn on_build_status_report_start(&self, w: &mut dyn Write) {
        writeln!(w, "{BUILD_STATUS_REPORT_START}").unwrap();
    }

    fn on_build_status_report_end(&self, w: &mut dyn Write) {
        writeln!(w, "{BUILD_STATUS_REPORT_END}").unwrap();
    }

    fn on_watch_status_report_start(&self) {
        let mut writer = self.current_write.clone();
        writeln!(writer, "{WATCH_STATUS_REPORT_START}").unwrap();
    }

    fn on_watch_status_report_end(&self) {
        let mut writer = self.current_write.clone();
        writeln!(writer, "{WATCH_STATUS_REPORT_END}").unwrap();
    }

    fn get_trace(
        &self,
        w: Box<dyn Write + Send>,
        locale: locale::Locale,
        use_package_json_cache: bool,
    ) -> Box<dyn Fn(&diagnostics::Message, Vec<serde_json::Value>) + Send + Sync> {
        let tracer = self.tracer.clone();
        let writer = Arc::new(Mutex::new(w));
        Box::new(move |msg, args| {
            writeln!(
                writer.lock().unwrap_or_else(|err| err.into_inner()),
                "{TRACE_START}"
            )
            .unwrap();
            // With tsc -b building projects in parallel we cannot serialize the package.json lookup trace
            // so trace as if it wasnt cached
            let str = msg.localize(
                locale.clone(),
                args.into_iter()
                    .map(|arg| Box::new(arg) as diagnostics::Any)
                    .collect(),
            );
            writeln!(
                writer.lock().unwrap_or_else(|err| err.into_inner()),
                "{}",
                tracer
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .sanitize_trace(&str, use_package_json_cache)
            )
            .unwrap();
            writeln!(
                writer.lock().unwrap_or_else(|err| err.into_inner()),
                "{TRACE_END}"
            )
            .unwrap();
        })
    }

    fn on_program(&self, program: &incremental::Program) {
        self.write_header_to_baseline(&self.program_baselines, program);
        let testing_data = program
            .get_testing_data()
            .expect("OnProgram should only be called with testing data");
        let compiler_program = program.get_program();
        let source_files = compiler_program.get_source_files();

        let mut program_baselines = self
            .program_baselines
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        program_baselines.push_str("SemanticDiagnostics::\n");
        for file in &source_files {
            let path = file.path();
            if let Some(diagnostics) = testing_data.semantic_diagnostics_per_file.get(&path) {
                if testing_data.refreshed_semantic_diagnostics.contains(&path)
                    || testing_data
                        .old_program_semantic_diagnostics_per_file
                        .get(&path)
                        .is_none_or(|old_diagnostics| old_diagnostics != diagnostics)
                {
                    program_baselines.push_str("*refresh*    ");
                    program_baselines.push_str(&file.file_name());
                    program_baselines.push('\n');
                }
            } else {
                program_baselines.push_str("*not cached* ");
                program_baselines.push_str(&file.file_name());
                program_baselines.push('\n');
            }
        }

        program_baselines.push_str("Signatures::\n");
        for file in &source_files {
            if let Some(kind) = testing_data.updated_signature_kinds.get(&file.path()) {
                match *kind {
                    value if value == incremental::SIGNATURE_UPDATE_KIND_COMPUTED_DTS => {
                        program_baselines.push_str("(computed .d.ts) ");
                    }
                    value if value == incremental::SIGNATURE_UPDATE_KIND_STORED_AT_EMIT => {
                        program_baselines.push_str("(stored at emit) ");
                    }
                    value if value == incremental::SIGNATURE_UPDATE_KIND_USED_VERSION as u8 => {
                        program_baselines.push_str("(used version)   ");
                    }
                    _ => continue,
                }
                program_baselines.push_str(&file.file_name());
                program_baselines.push('\n');
            }
        }
        drop(program_baselines);

        let mut files_without_include_reason = Vec::new();
        let mut file_not_in_program_with_include_reason = Vec::new();
        let include_reasons = compiler_program.get_include_reasons();
        for file in &source_files {
            if !include_reasons.contains_key(&file.path()) {
                files_without_include_reason.push(file.path().to_string());
            }
        }
        for path in include_reasons.keys() {
            if compiler_program
                .get_source_file_by_path(path.clone())
                .is_none()
                && !compiler_program.is_missing_path(path.clone())
            {
                file_not_in_program_with_include_reason.push(path.to_string());
            }
        }
        if !files_without_include_reason.is_empty()
            || !file_not_in_program_with_include_reason.is_empty()
        {
            self.write_header_to_baseline(&self.program_include_baselines, program);
            let mut program_include_baselines = self
                .program_include_baselines
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            program_include_baselines.push_str(
                "!!! Expected all files to have include reasons\nfilesWithoutIncludeReason::\n",
            );
            for file in files_without_include_reason {
                program_include_baselines.push_str("  ");
                program_include_baselines.push_str(&file);
                program_include_baselines.push('\n');
            }
            program_include_baselines.push_str("filesNotInProgramWithIncludeReason::\n");
            for file in file_not_in_program_with_include_reason {
                program_include_baselines.push_str("  ");
                program_include_baselines.push_str(&file);
                program_include_baselines.push('\n');
            }
        }
    }
}

impl TestSys {
    fn write_header_to_baseline(
        &self,
        builder: &Arc<Mutex<String>>,
        program: &incremental::Program,
    ) {
        let mut builder = builder.lock().unwrap_or_else(|err| err.into_inner());
        if !builder.is_empty() {
            builder.push('\n');
        }
        let config_file_path = program.options().config_file_path;
        if !config_file_path.is_empty() {
            builder.push_str(&tspath::get_relative_path_from_directory(
                &self.cwd,
                &config_file_path,
                &tspath::ComparePathsOptions {
                    use_case_sensitive_file_names: self.fs().use_case_sensitive_file_names(),
                    current_directory: self.get_current_directory(),
                },
            ));
            builder.push_str("::\n");
        }
    }
}

impl TestFsDiffer {
    pub fn baseline_fs_with_diff(&mut self, sys: &TestSys, baseline: &mut String) {
        let mut snap = HashMap::new();
        let mut diffs = BTreeMap::new();

        for (path, file) in sys.fs_from_file_map().entries() {
            if let Some(target) = file.symlink_target {
                let new_entry = fsbaselineutil::DiffEntry {
                    symlink_target: target,
                    ..Default::default()
                };
                snap.insert(path.clone(), new_entry.clone());
                self.add_fs_entry_diff(sys, &mut diffs, Some(&new_entry), &path);
            } else if file.is_regular {
                let content = fsbaselineutil::sanitize_internal_symbol_name(
                    &String::from_utf8_lossy(&file.data),
                );
                let new_entry = fsbaselineutil::DiffEntry {
                    content,
                    mtime: file.mtime,
                    is_written: sys.fs.written_files.has(&path),
                    symlink_target: String::new(),
                };
                snap.insert(path.clone(), new_entry.clone());
                self.add_fs_entry_diff(sys, &mut diffs, Some(&new_entry), &path);
            }
        }

        if let Some(serialized_diff) = &self.serialized_diff {
            for path in serialized_diff.snap.keys() {
                if sys.fs_from_file_map().get_file_info(path).is_none() {
                    self.add_fs_entry_diff(sys, &mut diffs, None, path);
                }
            }
        }

        let default_libs = default_libs_set(sys);
        self.serialized_diff = Some(fsbaselineutil::Snapshot { snap, default_libs });

        for (path, diff) in diffs {
            baseline.push_str(&format!("//// [{path}] {diff}\n"));
        }
        baseline.push('\n');
        for key in sys.fs.written_files.to_slice() {
            sys.fs.written_files.delete(&key);
        }
    }

    fn add_fs_entry_diff(
        &self,
        sys: &TestSys,
        diffs: &mut BTreeMap<String, String>,
        new_dir_content: Option<&fsbaselineutil::DiffEntry>,
        path: &str,
    ) {
        let old_dir_content = self
            .serialized_diff
            .as_ref()
            .and_then(|snapshot| snapshot.snap.get(path));
        let old_default_libs = self
            .serialized_diff
            .as_ref()
            .map(|snapshot| &snapshot.default_libs);
        let current_default_libs = default_libs_set(sys);

        match (old_dir_content, new_dir_content) {
            (None, Some(new_content)) => {
                if !current_default_libs.contains(path) {
                    if !new_content.symlink_target.is_empty() {
                        diffs.insert(
                            path.to_owned(),
                            format!("-> {} *new*", new_content.symlink_target),
                        );
                    } else {
                        diffs.insert(path.to_owned(), format!("*new* \n{}", new_content.content));
                    }
                }
            }
            (Some(_), None) => {
                diffs.insert(path.to_owned(), "*deleted*".to_owned());
            }
            (Some(old_content), Some(new_content)) => {
                if new_content.content != old_content.content {
                    diffs.insert(
                        path.to_owned(),
                        format!("*modified* \n{}", new_content.content),
                    );
                } else if new_content.is_written {
                    diffs.insert(path.to_owned(), "*rewrite with same content*".to_owned());
                } else if new_content.mtime != old_content.mtime {
                    diffs.insert(path.to_owned(), "*mTime changed*".to_owned());
                } else if old_default_libs.is_some_and(|libs| libs.contains(path))
                    && !current_default_libs.contains(path)
                {
                    diffs.insert(path.to_owned(), format!("*Lib*\n{}", new_content.content));
                }
            }
            (None, None) => {}
        }
    }
}

fn default_libs_set(sys: &TestSys) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Some(default_libs) = &sys.fs.default_libs {
        default_libs.range(|key| {
            set.insert(key.clone());
            true
        });
    }
    set
}

pub const FAKE_TIME_STAMP: &str = "HH:MM:SS AM";
pub const FAKE_DURATION: &str = "d.ddds";

pub const BUILD_STARTING_AT: &str = "build starting at ";
pub const BUILD_FINISHED_IN: &str = "build finished in ";
pub const LIST_FILE_START: &str = "!!! List files start";
pub const LIST_FILE_END: &str = "!!! List files end";
pub const STATISTICS_START: &str = "!!! Statistics start";
pub const STATISTICS_END: &str = "!!! Statistics end";
pub const BUILD_STATUS_REPORT_START: &str = "!!! Build Status Report Start";
pub const BUILD_STATUS_REPORT_END: &str = "!!! Build Status Report End";
pub const WATCH_STATUS_REPORT_START: &str = "!!! Watch Status Report Start";
pub const WATCH_STATUS_REPORT_END: &str = "!!! Watch Status Report End";
pub const TRACE_START: &str = "!!! Trace start";
pub const TRACE_END: &str = "!!! Trace end";

pub struct OutputSanitizer {
    pub for_comparing: bool,
    pub lines: Vec<String>,
    pub index: usize,
    pub output_lines: Vec<String>,
}

impl OutputSanitizer {
    pub fn add_output_line(&mut self, s: String) {
        let english_version =
            diagnostics::Version_0.localize(locale::DEFAULT, vec![Box::new(core::version())]);
        let fake_english_version = diagnostics::Version_0.localize(
            locale::DEFAULT,
            vec![Box::new(harnessutil::FAKE_TS_VERSION)],
        );
        let czech = locale::Locale::from("cs");
        let czech_version =
            diagnostics::Version_0.localize(czech.clone(), vec![Box::new(core::version())]);
        let fake_czech_version =
            diagnostics::Version_0.localize(czech, vec![Box::new(harnessutil::FAKE_TS_VERSION)]);
        let s = s.replace(
            &format!("'{}'", core::version()),
            &format!("'{}'", harnessutil::FAKE_TS_VERSION),
        );
        let s = s.replace(&english_version, &fake_english_version);
        let s = s.replace(&czech_version, &fake_czech_version);
        self.output_lines
            .push(fsbaselineutil::sanitize_internal_symbol_name(&s));
    }

    pub fn sanitize_build_status_time_stamp(&self) -> String {
        let status_line = &self.lines[self.index];
        let hh_separator = status_line.find(':').expect("Expected timestamp");
        assert!(hh_separator >= 2, "Expected timestamp");
        format!(
            "{}{}{}",
            &status_line[..hh_separator - 2],
            FAKE_TIME_STAMP,
            &status_line[hh_separator + FAKE_TIME_STAMP.len() - 2..]
        )
    }

    pub fn transform_lines(&mut self) -> String {
        while self.index < self.lines.len() {
            let line = self.lines[self.index].clone();
            if line.starts_with(BUILD_STARTING_AT) {
                if !self.for_comparing {
                    self.add_output_line(format!("{BUILD_STARTING_AT}{FAKE_TIME_STAMP}"));
                }
                self.index += 1;
                continue;
            }
            if line.starts_with(BUILD_FINISHED_IN) {
                if !self.for_comparing {
                    self.add_output_line(format!("{BUILD_FINISHED_IN}{FAKE_DURATION}"));
                }
                self.index += 1;
                continue;
            }
            if !self.add_or_skip_lines_for_comparing(LIST_FILE_START, LIST_FILE_END, false, None)
                && !self.add_or_skip_lines_for_comparing(
                    STATISTICS_START,
                    STATISTICS_END,
                    true,
                    None,
                )
                && !self.add_or_skip_lines_for_comparing(TRACE_START, TRACE_END, false, None)
                && !self.add_or_skip_lines_for_comparing(
                    BUILD_STATUS_REPORT_START,
                    BUILD_STATUS_REPORT_END,
                    false,
                    Some(OutputSanitizer::sanitize_build_status_time_stamp),
                )
                && !self.add_or_skip_lines_for_comparing(
                    WATCH_STATUS_REPORT_START,
                    WATCH_STATUS_REPORT_END,
                    false,
                    Some(OutputSanitizer::sanitize_build_status_time_stamp),
                )
            {
                self.add_output_line(line);
                self.index += 1;
            }
        }
        self.output_lines.join("\n")
    }

    pub fn add_or_skip_lines_for_comparing(
        &mut self,
        line_start: &str,
        line_end: &str,
        skip_even_if_not_comparing: bool,
        sanitize_first_line: Option<fn(&OutputSanitizer) -> String>,
    ) -> bool {
        if self.lines[self.index] != line_start {
            return false;
        }
        self.index += 1;
        let mut is_first_line = true;
        while self.index < self.lines.len() {
            if self.lines[self.index] == line_end {
                self.index += 1;
                return true;
            }
            if !self.for_comparing && !skip_even_if_not_comparing {
                let mut line = self.lines[self.index].clone();
                if is_first_line {
                    if let Some(sanitize_first_line) = sanitize_first_line {
                        line = sanitize_first_line(self);
                    }
                    is_first_line = false;
                }
                self.add_output_line(line);
            }
            self.index += 1;
        }
        panic!("Expected lineEnd{line_end} not found after {line_start}")
    }
}
