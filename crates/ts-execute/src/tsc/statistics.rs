use std::io::Write;
use std::time::Duration;

use ts_compiler as compiler;

use super::{CommandLineTesting, CompileTimes, EmitInput};

pub struct TableRow {
    pub name: String,
    pub value: String,
}

pub struct Table {
    pub rows: Vec<TableRow>,
}

pub enum TableValue {
    Duration(Duration),
    String(String),
}

impl From<Duration> for TableValue {
    fn from(value: Duration) -> Self {
        Self::Duration(value)
    }
}

impl From<String> for TableValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for TableValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<usize> for TableValue {
    fn from(value: usize) -> Self {
        Self::String(value.to_string())
    }
}

impl From<u64> for TableValue {
    fn from(value: u64) -> Self {
        Self::String(value.to_string())
    }
}

impl Table {
    pub fn add<V: Into<TableValue>>(&mut self, name: String, value: V) {
        let value = match value.into() {
            TableValue::Duration(d) => format_duration(d),
            TableValue::String(value) => value,
        };
        self.rows.push(TableRow { name, value });
    }

    pub fn print(&self, w: &mut dyn Write) {
        let mut name_width = 0;
        let mut value_width = 0;
        for r in &self.rows {
            name_width = name_width.max(r.name.len());
            value_width = value_width.max(r.value.len());
        }

        for r in &self.rows {
            let _ = writeln!(
                w,
                "{:<name_width$} {:>value_width$}",
                format!("{}:", r.name),
                r.value,
                name_width = name_width + 1,
                value_width = value_width
            );
        }
    }
}

pub fn format_duration(d: Duration) -> String {
    format!("{:.3}s", d.as_secs_f64())
}

pub fn identifier_count(p: &compiler::Program) -> usize {
    let mut count = 0;
    for file in p.source_files() {
        count += file.identifier_count() as usize;
    }
    count
}

#[derive(Default, Clone)]
pub struct Statistics {
    pub is_aggregate: bool,
    pub projects: usize,
    pub projects_built: usize,
    pub timestamp_updates: usize,
    pub files: usize,
    pub lines: usize,
    pub identifiers: usize,
    pub symbols: usize,
    pub types: usize,
    pub instantiations: usize,
    pub memory_used: u64,
    pub memory_allocs: u64,
    pub compile_times: Option<CompileTimes>,
}

pub fn statistics_from_program(input: &EmitInput, mem_stats: &MemoryStats) -> Statistics {
    let program = input.program_like.program();
    Statistics {
        files: program.source_files().len(),
        lines: program.line_count(),
        identifiers: program.identifier_count(),
        symbols: program.symbol_count(),
        types: program.type_count(),
        instantiations: program.instantiation_count(),
        memory_used: mem_stats.alloc,
        memory_allocs: mem_stats.mallocs,
        compile_times: Some(input.compile_times.clone()),
        ..Default::default()
    }
}

impl Statistics {
    pub fn report(&self, mut w: Box<dyn Write>, testing: Option<CommandLineTesting>) {
        if let Some(testing) = &testing {
            testing.on_statistics_start(&mut *w);
        }
        let mut table = Table { rows: Vec::new() };
        let mut prefix = String::new();

        if self.is_aggregate {
            prefix = "Aggregate ".to_owned();
            table.add("Projects in scope".to_owned(), self.projects);
            table.add("Projects built".to_owned(), self.projects_built);
            table.add("Timestamps only updates".to_owned(), self.timestamp_updates);
        }
        table.add(format!("{prefix}Files"), self.files);
        table.add(format!("{prefix}Lines"), self.lines);
        table.add(format!("{prefix}Identifiers"), self.identifiers);
        table.add(format!("{prefix}Symbols"), self.symbols);
        table.add(format!("{prefix}Types"), self.types);
        table.add(format!("{prefix}Instantiations"), self.instantiations);
        table.add(
            format!("{prefix}Memory used"),
            format!("{}K", self.memory_used / 1024),
        );
        table.add(
            format!("{prefix}Memory allocs"),
            self.memory_allocs.to_string(),
        );
        let compile_times = self.compile_times.as_ref().expect("compileTimes is nil");
        if compile_times.config_time != Duration::ZERO {
            table.add(format!("{prefix}Config time"), compile_times.config_time);
        }
        if compile_times.build_info_read_time != Duration::ZERO {
            table.add(
                format!("{prefix}BuildInfo read time"),
                compile_times.build_info_read_time,
            );
        }
        table.add(format!("{prefix}Parse time"), compile_times.parse_time);
        if compile_times.bind_time != Duration::ZERO {
            table.add(format!("{prefix}Bind time"), compile_times.bind_time);
        }
        if compile_times.check_time != Duration::ZERO {
            table.add(format!("{prefix}Check time"), compile_times.check_time);
        }
        if compile_times.emit_time != Duration::ZERO {
            table.add(format!("{prefix}Emit time"), compile_times.emit_time);
        }
        if compile_times.changes_compute_time != Duration::ZERO {
            table.add(
                format!("{prefix}Changes compute time"),
                compile_times.changes_compute_time,
            );
        }
        table.add(format!("{prefix}Total time"), compile_times.total_time);
        table.print(&mut *w);
        if let Some(testing) = testing {
            testing.on_statistics_end(&mut *w);
        }
    }

    pub fn aggregate(&mut self, stat: &Statistics) {
        self.is_aggregate = true;
        if self.compile_times.is_none() {
            self.compile_times = Some(CompileTimes::default());
        }
        // Aggregate statistics
        self.files += stat.files;
        self.lines += stat.lines;
        self.identifiers += stat.identifiers;
        self.symbols += stat.symbols;
        self.types += stat.types;
        self.instantiations += stat.instantiations;
        self.memory_used += stat.memory_used;
        self.memory_allocs += stat.memory_allocs;
        let this = self.compile_times.as_mut().unwrap();
        let that = stat.compile_times.as_ref().expect("compileTimes is nil");
        this.config_time += that.config_time;
        this.build_info_read_time += that.build_info_read_time;
        this.parse_time += that.parse_time;
        this.bind_time += that.bind_time;
        this.check_time += that.check_time;
        this.emit_time += that.emit_time;
        this.changes_compute_time += that.changes_compute_time;
    }

    pub fn set_total_time(&mut self, total_time: Duration) {
        if self.compile_times.is_none() {
            self.compile_times = Some(CompileTimes::default());
        }
        self.compile_times.as_mut().unwrap().total_time = total_time;
    }
}

pub struct MemoryStats {
    pub alloc: u64,
    pub mallocs: u64,
}

impl MemoryStats {
    pub fn read_after_settling() -> Self {
        let alloc = resident_memory_bytes().unwrap_or(0);
        Self { alloc, mallocs: 0 }
    }
}

#[cfg(target_os = "linux")]
fn resident_memory_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let rss = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?;
    Some(rss.saturating_mul(1024))
}

#[cfg(not(target_os = "linux"))]
fn resident_memory_bytes() -> Option<u64> {
    None
}
