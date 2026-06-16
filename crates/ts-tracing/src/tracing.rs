use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::Instant as StdInstant;

use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use ts_ast as ast;
use ts_scanner as scanner;
use ts_tspath as tspath;
use ts_vfs as vfs;
use xxhash_rust::xxh3::Xxh3;

pub type Any = Value;

pub trait IntoTraceArgs {
    fn into_trace_args(self) -> HashMap<String, Any>;
}

pub fn args<T: IntoTraceArgs>(args: T) -> HashMap<String, Any> {
    args.into_trace_args()
}

impl<K, V> IntoTraceArgs for HashMap<K, V>
where
    K: Into<String> + Eq + Hash,
    V: Into<Any>,
{
    fn into_trace_args(self) -> HashMap<String, Any> {
        self.into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect()
    }
}

impl<const N: usize> IntoTraceArgs for [(&str, String); N] {
    fn into_trace_args(self) -> HashMap<String, Any> {
        self.into_iter()
            .map(|(key, value)| (key.to_string(), value.into()))
            .collect()
    }
}

impl<const N: usize> IntoTraceArgs for &[(&str, String); N] {
    fn into_trace_args(self) -> HashMap<String, Any> {
        self.iter()
            .map(|(key, value)| ((*key).to_string(), value.clone().into()))
            .collect()
    }
}

impl<const N: usize> IntoTraceArgs for [(&str, &str); N] {
    fn into_trace_args(self) -> HashMap<String, Any> {
        self.into_iter()
            .map(|(key, value)| (key.to_string(), value.into()))
            .collect()
    }
}

impl<const N: usize> IntoTraceArgs for &[(&str, &str); N] {
    fn into_trace_args(self) -> HashMap<String, Any> {
        self.iter()
            .map(|(key, value)| ((*key).to_string(), (*value).into()))
            .collect()
    }
}

impl<const N: usize> IntoTraceArgs for [(&str, Any); N] {
    fn into_trace_args(self) -> HashMap<String, Any> {
        self.into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect()
    }
}

pub trait Tracer {
    fn record_type(&mut self, typ: Box<dyn TracedType + '_>);
    fn dump_types(&mut self) -> Result<(), String>;
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct Location {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<LineAndChar>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<LineAndChar>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct LineAndChar {
    pub line: i32,
    pub character: i32,
}

pub trait TracedType {
    fn id(&self) -> u32;
    fn format_flags(&self) -> Vec<String>;
    fn is_conditional(&self) -> bool;
    fn symbol(&self) -> Option<ast::SymbolIdentity>;
    fn alias_symbol(&self) -> Option<ast::SymbolIdentity>;
    fn symbol_name(&self, symbol: ast::SymbolIdentity) -> Option<String>;
    fn first_symbol_declaration(&self, symbol: ast::SymbolIdentity) -> Option<ast::Node>;
    fn alias_type_arguments(&self) -> Vec<Box<dyn TracedType + '_>>;
    fn intrinsic_name(&self) -> String;
    fn union_types(&self) -> Vec<Box<dyn TracedType + '_>>;
    fn intersection_types(&self) -> Vec<Box<dyn TracedType + '_>>;
    fn index_type(&self) -> Option<Box<dyn TracedType + '_>>;
    fn indexed_access_object_type(&self) -> Option<Box<dyn TracedType + '_>>;
    fn indexed_access_index_type(&self) -> Option<Box<dyn TracedType + '_>>;
    fn conditional_check_type(&self) -> Option<Box<dyn TracedType + '_>>;
    fn conditional_extends_type(&self) -> Option<Box<dyn TracedType + '_>>;
    fn conditional_true_type(&self) -> Option<Box<dyn TracedType + '_>>;
    fn conditional_false_type(&self) -> Option<Box<dyn TracedType + '_>>;
    fn substitution_base_type(&self) -> Option<Box<dyn TracedType + '_>>;
    fn substitution_constraint_type(&self) -> Option<Box<dyn TracedType + '_>>;
    fn reference_target(&self) -> Option<Box<dyn TracedType + '_>>;
    fn reference_type_arguments(&self) -> Vec<Box<dyn TracedType + '_>>;
    fn reference_node(&self) -> Option<ast::Node>;
    fn reverse_mapped_source_type(&self) -> Option<Box<dyn TracedType + '_>>;
    fn reverse_mapped_mapped_type(&self) -> Option<Box<dyn TracedType + '_>>;
    fn reverse_mapped_constraint_type(&self) -> Option<Box<dyn TracedType + '_>>;
    fn evolving_array_element_type(&self) -> Option<Box<dyn TracedType + '_>>;
    fn evolving_array_final_type(&self) -> Option<Box<dyn TracedType + '_>>;
    fn is_tuple(&self) -> bool;
    fn pattern(&self) -> Option<ast::Node>;
    fn get_location(&self, node: ast::Node) -> Option<Location>;
    fn recursion_identity(&self) -> Option<usize>;
    fn display(&self) -> String;
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceRecord {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub config_file_path: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub trace_path: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub types_path: String,
    #[serde(rename = "checkerId")]
    pub checker_id: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct TraceEvent {
    #[serde(rename = "pid")]
    pub pid: i32,
    #[serde(rename = "tid")]
    pub tid: i32,
    #[serde(rename = "ph")]
    pub ph: String,
    pub cat: String,
    #[serde(rename = "ts")]
    pub ts: f64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub s: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dur: Option<f64>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(serialize_with = "serialize_trace_args")]
    pub args: HashMap<String, Any>,
}

fn serialize_trace_args<S>(args: &HashMap<String, Any>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut entries = args.iter().collect::<Vec<_>>();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));

    let mut map = serializer.serialize_map(Some(entries.len()))?;
    for (key, value) in entries {
        map.serialize_entry(key, value)?;
    }
    map.end()
}

pub const TRACE_FILE_NAME: &str = "trace.json";
pub const MAIN_THREAD_ID: i32 = 1;
pub const FIRST_SYNTHETIC_THREAD_ID: i32 = 2;
pub const FIRST_FILE_THREAD_ID: i32 = 1_000_000;
pub const FILE_THREAD_ID_HASH_RANGE: i32 = 1_000_000_000;
pub const FLUSH_THRESHOLD: usize = 256 * 1024;
pub const SAMPLE_INTERVAL_MICROS: f64 = 10_000.0;

const TRACE_THREAD_ARG_KEYS: [&str; 5] = [
    "path",
    "fileName",
    "containingFileName",
    "jsFilePath",
    "declarationFilePath",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Parse,
    Program,
    Bind,
    Check,
    CheckTypes,
    Emit,
    Session,
}

pub const PHASE_PARSE: Phase = Phase::Parse;
pub const PHASE_PROGRAM: Phase = Phase::Program;
pub const PHASE_BIND: Phase = Phase::Bind;
pub const PHASE_CHECK: Phase = Phase::Check;
pub const PHASE_CHECK_TYPES: Phase = Phase::CheckTypes;
pub const PHASE_EMIT: Phase = Phase::Emit;
pub const PHASE_SESSION: Phase = Phase::Session;

impl Phase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Phase::Parse => "parse",
            Phase::Program => "program",
            Phase::Bind => "bind",
            Phase::Check => "check",
            Phase::CheckTypes => "checkTypes",
            Phase::Emit => "emit",
            Phase::Session => "session",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TraceThreadKind {
    Checker,
    File,
}

impl TraceThreadKind {
    fn as_str(&self) -> &'static str {
        match self {
            TraceThreadKind::Checker => "checker",
            TraceThreadKind::File => "file",
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TraceThreadKey {
    pub kind: TraceThreadKind,
    pub text: String,
    pub index: i32,
    pub has_index: bool,
}

impl TraceThreadKey {
    pub fn default_thread_id(&self) -> i32 {
        if self.kind == TraceThreadKind::Checker && self.has_index && self.index >= 0 {
            return FIRST_SYNTHETIC_THREAD_ID + self.index;
        }
        stable_trace_thread_id_for_key(self)
    }

    pub fn display_name(&self) -> String {
        if self.has_index {
            format!("{}:{}", self.kind.as_str(), self.index)
        } else {
            format!("{}:{}", self.kind.as_str(), self.text)
        }
    }
}

trait TraceFileSystem: Send + Sync {
    fn write_file(&self, path: &str, text: &str) -> Result<(), String>;
    fn append_file(&self, path: &str, text: &str) -> Result<(), String>;
}

struct OwnedTraceFileSystem<F: vfs::Fs + Clone + Send + Sync + 'static>(F);

impl<F: vfs::Fs + Clone + Send + Sync + 'static> TraceFileSystem for OwnedTraceFileSystem<F> {
    fn write_file(&self, path: &str, text: &str) -> Result<(), String> {
        self.0.write_file(path, text).map_err(|err| err.to_string())
    }

    fn append_file(&self, path: &str, text: &str) -> Result<(), String> {
        self.0
            .append_file(path, text)
            .map_err(|err| err.to_string())
    }
}

#[derive(Clone)]
pub struct Tracing {
    fs: Arc<dyn TraceFileSystem>,
    pub trace_dir: String,
    pub trace_path: String,
    pub config_file_path: String,
    pub legend: Vec<TraceRecord>,
    pub tracers: Vec<TypeTracer>,
    pub trace_content: String,
    pub trace_started: bool,
    pub thread_ids: HashMap<TraceThreadKey, i32>,
    pub thread_keys: HashMap<i32, TraceThreadKey>,
    pub metadata_ts: f64,
    pub deterministic: bool,
    pub timestamp_counter: u64,
    pub start_time: StdInstant,
    pub flush_err: Option<String>,
}

pub fn start_tracing<F: vfs::Fs + Clone + Send + Sync + 'static>(
    fs: F,
    trace_dir: &str,
    config_file_path: &str,
    deterministic: bool,
) -> Result<Tracing, String> {
    let fs: Arc<dyn TraceFileSystem> = Arc::new(OwnedTraceFileSystem(fs));
    let mut tracing = Tracing {
        fs,
        trace_dir: trace_dir.to_string(),
        trace_path: tspath::combine_paths(trace_dir, &[TRACE_FILE_NAME]),
        config_file_path: config_file_path.to_string(),
        legend: Vec::new(),
        tracers: Vec::new(),
        trace_content: String::new(),
        trace_started: true,
        thread_ids: HashMap::new(),
        thread_keys: HashMap::new(),
        metadata_ts: 0.0,
        deterministic,
        timestamp_counter: 0,
        start_time: StdInstant::now(),
        flush_err: None,
    };
    tracing.trace_content.push_str("[\n");
    let meta_ts = tracing.timestamp();
    tracing.metadata_ts = meta_ts;
    tracing.write_event(TraceEvent {
        pid: 1,
        tid: MAIN_THREAD_ID,
        ph: "M".to_string(),
        cat: "__metadata".to_string(),
        ts: meta_ts,
        name: "process_name".to_string(),
        args: HashMap::from([("name".to_string(), "tsgo".into())]),
        ..TraceEvent::default()
    });
    tracing.trace_content.push_str(",\n");
    tracing.write_event(TraceEvent {
        pid: 1,
        tid: MAIN_THREAD_ID,
        ph: "M".to_string(),
        cat: "__metadata".to_string(),
        ts: meta_ts,
        name: "thread_name".to_string(),
        args: HashMap::from([("name".to_string(), "Main".into())]),
        ..TraceEvent::default()
    });
    tracing.trace_content.push_str(",\n");
    tracing.write_event(TraceEvent {
        pid: 1,
        tid: MAIN_THREAD_ID,
        ph: "M".to_string(),
        cat: "disabled-by-default-devtools.timeline".to_string(),
        ts: meta_ts,
        name: "TracingStartedInBrowser".to_string(),
        ..TraceEvent::default()
    });
    tracing
        .fs
        .write_file(&tracing.trace_path, &tracing.trace_content)
        .map_err(|err| format!("failed to write trace file header: {err}"))?;
    tracing.trace_content.clear();
    Ok(tracing)
}

impl Tracing {
    pub fn timestamp(&mut self) -> f64 {
        if self.deterministic {
            self.timestamp_counter += 1;
            self.timestamp_counter as f64
        } else {
            self.start_time.elapsed().as_nanos() as f64 / 1000.0
        }
    }

    pub fn write_event(&mut self, event: TraceEvent) {
        write_event_to(&mut self.trace_content, &event);
    }

    pub fn maybe_flush_locked(&mut self) {
        if self.flush_err.is_some() {
            self.trace_content.clear();
            return;
        }
        if self.trace_content.len() < FLUSH_THRESHOLD {
            return;
        }
        if let Err(err) = self.fs.append_file(&self.trace_path, &self.trace_content) {
            self.flush_err = Some(format!("failed to flush trace file: {err}"));
        }
        self.trace_content.clear();
    }

    pub fn instant<A: IntoTraceArgs>(&mut self, phase: Phase, name: &str, args: A) {
        if !self.trace_started {
            return;
        }
        let args = args.into_trace_args();
        let ts = self.timestamp();
        // PORT NOTE: reshaped for borrowck; compute the thread id before building the event.
        let tid = self.thread_id_locked(&args);
        self.trace_content.push_str(",\n");
        self.write_event(TraceEvent {
            pid: 1,
            tid,
            ph: "I".to_string(),
            cat: phase.as_str().to_string(),
            ts,
            name: name.to_string(),
            s: "g".to_string(),
            args,
            ..TraceEvent::default()
        });
        self.maybe_flush_locked();
    }

    pub fn push<A>(
        &mut self,
        phase: Phase,
        name: &str,
        args: A,
        separate_begin_and_end: bool,
    ) -> Box<dyn FnOnce(&mut Tracing)>
    where
        A: IntoTraceArgs,
    {
        if !self.trace_started {
            return Box::new(|_| {});
        }
        let args = args.into_trace_args();
        if separate_begin_and_end {
            let ts = self.timestamp();
            let tid = self.thread_id_locked(&args);
            self.trace_content.push_str(",\n");
            self.write_event(TraceEvent {
                pid: 1,
                tid,
                ph: "B".to_string(),
                cat: phase.as_str().to_string(),
                ts,
                name: name.to_string(),
                args: args.clone(),
                ..TraceEvent::default()
            });
            self.maybe_flush_locked();
            let phase = phase.as_str().to_string();
            let name = name.to_string();
            return Box::new(move |tracing| {
                if !tracing.trace_started {
                    return;
                }
                let ts = tracing.timestamp();
                tracing.trace_content.push_str(",\n");
                tracing.write_event(TraceEvent {
                    pid: 1,
                    tid,
                    ph: "E".to_string(),
                    cat: phase,
                    ts,
                    name,
                    args,
                    ..TraceEvent::default()
                });
                tracing.maybe_flush_locked();
            });
        }

        if self.deterministic {
            return Box::new(|_| {});
        }

        let start_time = StdInstant::now();
        let tracing_start_time = self.start_time;
        let phase = phase.as_str().to_string();
        let name = name.to_string();
        Box::new(move |tracing| {
            let dur = start_time.elapsed().as_nanos() as f64 / 1000.0;
            let start_micros =
                start_time.duration_since(tracing_start_time).as_nanos() as f64 / 1000.0;
            if SAMPLE_INTERVAL_MICROS - (start_micros % SAMPLE_INTERVAL_MICROS) > dur {
                return;
            }
            if !tracing.trace_started {
                return;
            }

            let tid = tracing.thread_id_locked(&args);
            tracing.trace_content.push_str(",\n");
            tracing.write_event(TraceEvent {
                pid: 1,
                tid,
                ph: "X".to_string(),
                cat: phase,
                ts: start_micros,
                name,
                dur: Some(dur),
                args,
                ..TraceEvent::default()
            });
            tracing.maybe_flush_locked();
        })
    }

    pub fn thread_id_locked(&mut self, args: &HashMap<String, Any>) -> i32 {
        let Some(key) = trace_thread_key_from_args(args) else {
            return MAIN_THREAD_ID;
        };

        if let Some(tid) = self.thread_ids.get(&key) {
            return *tid;
        }

        let mut tid = key.default_thread_id();
        while self
            .thread_keys
            .get(&tid)
            .is_some_and(|existing_key| existing_key != &key)
        {
            tid += 1;
        }

        self.thread_ids.insert(key.clone(), tid);
        self.thread_keys.insert(tid, key.clone());
        self.write_thread_name_event_locked(tid, &key.display_name());
        tid
    }

    pub fn write_thread_name_event_locked(&mut self, tid: i32, name: &str) {
        self.trace_content.push_str(",\n");
        self.write_event(TraceEvent {
            pid: 1,
            tid,
            ph: "M".to_string(),
            cat: "__metadata".to_string(),
            ts: self.metadata_ts,
            name: "thread_name".to_string(),
            args: HashMap::from([("name".to_string(), name.into())]),
            ..TraceEvent::default()
        });
    }

    pub fn new_type_tracer(&mut self, checker_index: i32) -> TypeTracer {
        let types_path =
            tspath::combine_paths(&self.trace_dir, &[&format!("types_{checker_index}.json")]);
        let tracer = TypeTracer {
            data: Arc::new(Mutex::new(TypeTracerData {
                fs: self.fs.clone(),
                checker_index,
                types_path: types_path.clone(),
                types: Vec::new(),
                recursion_identity_map: HashMap::new(),
            })),
        };
        self.tracers.push(tracer.clone());
        self.legend.push(TraceRecord {
            config_file_path: self.config_file_path.clone(),
            trace_path: self.trace_path.clone(),
            types_path,
            checker_id: checker_index,
        });
        tracer
    }

    pub fn stop_tracing(&mut self) -> Result<(), String> {
        for tracer in &mut self.tracers {
            tracer.dump_types().map_err(|err| {
                format!(
                    "failed to dump types for checker {}: {err}",
                    tracer.checker_index()
                )
            })?;
        }
        if self.trace_started {
            if let Some(err) = self.flush_err.take() {
                self.trace_content.clear();
                self.trace_started = false;
                return Err(err);
            }
            self.fs
                .append_file(&self.trace_path, &(self.trace_content.clone() + "\n]\n"))
                .map_err(|err| format!("failed to write trace file: {err}"))?;
            self.trace_content.clear();
            self.trace_started = false;
        }
        self.legend.sort_by(|a, b| a.types_path.cmp(&b.types_path));
        let legend_data = serde_json::to_string_pretty(&self.legend)
            .map_err(|err| format!("failed to marshal legend file: {err}"))?;
        self.fs
            .write_file(
                &tspath::combine_paths(&self.trace_dir, &["legend.json"]),
                &legend_data,
            )
            .map_err(|err| format!("failed to write legend file: {err}"))?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct TypeTracer {
    data: Arc<Mutex<TypeTracerData>>,
}

struct TypeTracerData {
    fs: Arc<dyn TraceFileSystem>,
    checker_index: i32,
    types_path: String,
    types: Vec<TypeDescriptor>,
    recursion_identity_map: HashMap<usize, i32>,
}

impl TypeTracer {
    fn checker_index(&self) -> i32 {
        self.data.lock().unwrap().checker_index
    }
}

impl Tracer for TypeTracer {
    fn record_type(&mut self, typ: Box<dyn TracedType + '_>) {
        let mut data = self.data.lock().unwrap();
        let descriptor = build_type_descriptor(&*typ, &mut data.recursion_identity_map);
        data.types.push(descriptor);
    }

    fn dump_types(&mut self) -> Result<(), String> {
        let (fs, types_path, descriptors) = {
            let data = self.data.lock().unwrap();
            (data.fs.clone(), data.types_path.clone(), data.types.clone())
        };

        if descriptors.is_empty() {
            return Ok(());
        }

        let mut output = String::new();
        output.push('[');
        for (index, descriptor) in descriptors.iter().enumerate() {
            serde_json::to_writer(WriteAdapter(&mut output), descriptor)
                .map_err(|err| format!("failed to marshal type {}: {err}", descriptor.id))?;
            if index < descriptors.len() - 1 {
                output.push_str(",\n");
            }
        }
        output.push_str("]\n");

        fs.write_file(&types_path, &output)
    }
}

struct WriteAdapter<'a>(&'a mut String);

impl std::io::Write for WriteAdapter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = std::str::from_utf8(buf)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        self.0.push_str(text);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDescriptor {
    pub id: u32,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub intrinsic_name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub symbol_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recursion_id: Option<i32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_tuple: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub union_types: Vec<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub intersection_types: Vec<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub alias_type_arguments: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyof_type: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexed_access_object_type: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexed_access_index_type: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditional_check_type: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditional_extends_type: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditional_true_type: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditional_false_type: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub substitution_base_type: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint_type: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instantiated_type: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub type_arguments: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_location: Option<Location>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reverse_mapped_source_type: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reverse_mapped_mapped_type: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reverse_mapped_constraint_type: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evolving_array_element_type: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evolving_array_final_type: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructuring_pattern: Option<Location>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_declaration: Option<Location>,
    pub flags: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub display: String,
}

fn build_type_descriptor(
    typ: &dyn TracedType,
    recursion_identity_map: &mut HashMap<usize, i32>,
) -> TypeDescriptor {
    let mut desc = TypeDescriptor {
        id: typ.id(),
        flags: typ.format_flags(),
        ..TypeDescriptor::default()
    };

    if let Some(identity) = typ.recursion_identity() {
        let next_token = recursion_identity_map.len() as i32;
        let token = *recursion_identity_map.entry(identity).or_insert(next_token);
        desc.recursion_id = Some(token);
    }

    if !typ.intrinsic_name().is_empty() {
        desc.intrinsic_name = typ.intrinsic_name();
    }

    if let Some(name) = typ
        .alias_symbol()
        .and_then(|symbol| typ.symbol_name(symbol))
    {
        desc.symbol_name = ast::escape_all_internal_symbol_names(&name);
    } else if let Some(name) = typ.symbol().and_then(|symbol| typ.symbol_name(symbol)) {
        desc.symbol_name = ast::escape_all_internal_symbol_names(&name);
    }

    desc.is_tuple = typ.is_tuple();
    desc.union_types = map_type_ids(&typ.union_types());
    desc.intersection_types = map_type_ids(&typ.intersection_types());
    desc.alias_type_arguments = map_type_ids(&typ.alias_type_arguments());
    desc.keyof_type = typ.index_type().map(|typ| typ.id());
    desc.indexed_access_object_type = typ.indexed_access_object_type().map(|typ| typ.id());
    desc.indexed_access_index_type = typ.indexed_access_index_type().map(|typ| typ.id());

    if typ.is_conditional() {
        desc.conditional_check_type = typ.conditional_check_type().map(|typ| typ.id());
        desc.conditional_extends_type = typ.conditional_extends_type().map(|typ| typ.id());
        desc.conditional_true_type = Some(
            typ.conditional_true_type()
                .map(|typ| typ.id() as i32)
                .unwrap_or(-1),
        );
        desc.conditional_false_type = Some(
            typ.conditional_false_type()
                .map(|typ| typ.id() as i32)
                .unwrap_or(-1),
        );
    }

    desc.substitution_base_type = typ.substitution_base_type().map(|typ| typ.id());
    desc.constraint_type = typ.substitution_constraint_type().map(|typ| typ.id());
    desc.instantiated_type = typ.reference_target().map(|typ| typ.id());
    desc.type_arguments = map_type_ids(&typ.reference_type_arguments());
    desc.reference_location = typ.reference_node().and_then(|node| typ.get_location(node));
    desc.reverse_mapped_source_type = typ.reverse_mapped_source_type().map(|typ| typ.id());
    desc.reverse_mapped_mapped_type = typ.reverse_mapped_mapped_type().map(|typ| typ.id());
    desc.reverse_mapped_constraint_type = typ.reverse_mapped_constraint_type().map(|typ| typ.id());
    desc.evolving_array_element_type = typ.evolving_array_element_type().map(|typ| typ.id());
    desc.evolving_array_final_type = typ.evolving_array_final_type().map(|typ| typ.id());
    desc.destructuring_pattern = typ.pattern().and_then(|node| typ.get_location(node));

    if let Some(declaration) = typ
        .alias_symbol()
        .or_else(|| typ.symbol())
        .and_then(|symbol| typ.first_symbol_declaration(symbol))
    {
        desc.first_declaration = typ.get_location(declaration);
    }

    if !typ.display().is_empty() {
        desc.display = typ.display();
    }

    desc
}

pub fn map_type_ids(types: &[Box<dyn TracedType + '_>]) -> Vec<u32> {
    if types.is_empty() {
        return Vec::new();
    }
    types.iter().map(|typ| typ.id()).collect()
}

pub fn get_location(node: ast::Node, file: &ast::SourceFile) -> Option<Location> {
    let start_pos = scanner::get_token_pos_of_node(&node, file, false);
    let (start_line, start_char) =
        scanner::get_ecma_line_and_utf16_character_of_position(file, start_pos);
    let end_pos = file.store().loc(node).end().max(0) as usize;
    let (end_line, end_char) =
        scanner::get_ecma_line_and_utf16_character_of_position(file, end_pos);

    Some(Location {
        path: tspath::to_path(&file.file_name(), "", false).to_string(),
        start: Some(LineAndChar {
            line: start_line as i32 + 1,
            character: start_char as i32 + 1,
        }),
        end: Some(LineAndChar {
            line: end_line as i32 + 1,
            character: end_char as i32 + 1,
        }),
    })
}

pub fn trace_thread_key_from_args(args: &HashMap<String, Any>) -> Option<TraceThreadKey> {
    if args.is_empty() {
        return None;
    }

    if let Some(checker_id) = args.get("checkerId").and_then(value_as_i32) {
        return Some(TraceThreadKey {
            kind: TraceThreadKind::Checker,
            text: String::new(),
            index: checker_id,
            has_index: true,
        });
    }

    for key in TRACE_THREAD_ARG_KEYS {
        if let Some(path) = args.get(key).and_then(Value::as_str)
            && !path.is_empty()
        {
            return Some(TraceThreadKey {
                kind: TraceThreadKind::File,
                text: path.to_string(),
                index: 0,
                has_index: false,
            });
        }
    }

    None
}

fn value_as_i32(value: &Value) -> Option<i32> {
    value
        .as_i64()
        .and_then(|value| i32::try_from(value).ok())
        .or_else(|| value.as_u64().and_then(|value| i32::try_from(value).ok()))
}

pub fn stable_trace_thread_id(kind: &str, text: &str) -> i32 {
    stable_trace_thread_id_for_key(&TraceThreadKey {
        kind: match kind {
            "checker" => TraceThreadKind::Checker,
            _ => TraceThreadKind::File,
        },
        text: text.to_string(),
        index: 0,
        has_index: false,
    })
}

pub fn stable_trace_thread_id_for_key(key: &TraceThreadKey) -> i32 {
    let mut hash = Xxh3::new();
    hash.write(key.kind.as_str().as_bytes());
    hash.write(b":");
    if key.has_index {
        hash.write(key.index.to_string().as_bytes());
    } else {
        hash.write(key.text.as_bytes());
    }
    FIRST_FILE_THREAD_ID + (hash.finish() % FILE_THREAD_ID_HASH_RANGE as u64) as i32
}

fn write_event_to(output: &mut String, event: &TraceEvent) {
    let text = serde_json::to_string(event).expect("failed to marshal trace event");
    output.push_str(&text);
}
