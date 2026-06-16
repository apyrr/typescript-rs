#![forbid(unsafe_code)]
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

mod runtime_pprof {
    use std::fs::File;
    use std::io::{self, Write};

    pub struct CpuProfile {
        file: File,
    }

    impl CpuProfile {
        pub fn start(file: File) -> io::Result<Self> {
            Ok(Self { file })
        }

        pub fn stop(mut self) -> io::Result<()> {
            self.file.flush()
        }
    }

    #[derive(Clone, Copy)]
    pub enum ProfileName {
        Allocs,
        Heap,
    }

    pub struct Profile {
        name: ProfileName,
    }

    impl Profile {
        pub fn write_to(&self, writer: &mut dyn Write, _debug: i32) -> io::Result<()> {
            let label = match self.name {
                ProfileName::Allocs => "allocs",
                ProfileName::Heap => "heap",
            };
            writeln!(
                writer,
                "# {label} profiling is not available in this Rust port"
            )
        }
    }

    pub fn lookup(name: ProfileName) -> Profile {
        Profile { name }
    }

    pub fn run_gc() {}
}

pub struct ProfileSession<W: Write> {
    cpu_file_path: PathBuf,
    mem_file_path: PathBuf,
    cpu_file: Option<File>,
    cpu_profile: Option<runtime_pprof::CpuProfile>,
    log_writer: W,
    stopped: bool,
}

// BeginProfiling starts CPU and memory profiling, writing the profiles to the specified directory.
pub fn begin_profiling<W: Write>(
    profile_dir: impl AsRef<Path>,
    log_writer: W,
) -> ProfileSession<W> {
    fs::create_dir_all(&profile_dir).unwrap_or_else(|err| panic!("{err}"));

    let pid = std::process::id();

    let cpu_profile_path = profile_dir.as_ref().join(format!("{pid}-cpuprofile.pb.gz"));
    let mem_profile_path = profile_dir.as_ref().join(format!("{pid}-memprofile.pb.gz"));
    let cpu_file = File::create(&cpu_profile_path).unwrap_or_else(|err| panic!("{err}"));

    let cpu_profile = runtime_pprof::CpuProfile::start(
        cpu_file.try_clone().unwrap_or_else(|err| panic!("{err}")),
    )
    .unwrap_or_else(|err| panic!("{err}"));

    ProfileSession {
        cpu_file_path: cpu_profile_path,
        mem_file_path: mem_profile_path,
        cpu_file: Some(cpu_file),
        cpu_profile: Some(cpu_profile),
        log_writer,
        stopped: false,
    }
}

impl<W: Write> ProfileSession<W> {
    pub fn stop(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;

        if let Some(cpu_profile) = self.cpu_profile.take() {
            cpu_profile.stop().unwrap_or_else(|err| panic!("{err}"));
        }
        if let Some(cpu_file) = self.cpu_file.take() {
            drop(cpu_file);
        }

        if !self.mem_file_path.as_os_str().is_empty() {
            let mut mem_file =
                File::create(&self.mem_file_path).unwrap_or_else(|err| panic!("{err}"));
            runtime_pprof::lookup(runtime_pprof::ProfileName::Allocs)
                .write_to(&mut mem_file, 0)
                .unwrap_or_else(|err| panic!("{err}"));
            drop(mem_file);
            writeln!(
                self.log_writer,
                "Memory profile: {}",
                self.mem_file_path.display()
            )
            .unwrap_or_else(|err| panic!("{err}"));
        }

        writeln!(
            self.log_writer,
            "CPU profile: {}",
            self.cpu_file_path.display()
        )
        .unwrap_or_else(|err| panic!("{err}"));
    }
}

// CPUProfiler manages on-demand CPU profiling.
#[derive(Default)]
pub struct CpuProfiler {
    session: Mutex<Option<ProfileSession<io::Sink>>>,
}

impl CpuProfiler {
    // StartCPUProfile starts CPU profiling, writing to the specified directory when stopped.
    pub fn start_cpu_profile(&self, profile_dir: impl AsRef<Path>) -> io::Result<()> {
        let mut session = self.session.lock().unwrap_or_else(|err| err.into_inner());

        if session.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "CPU profiling already in progress",
            ));
        }

        fs::create_dir_all(&profile_dir).map_err(|err| {
            io::Error::new(
                err.kind(),
                format!("failed to create profile directory: {err}"),
            )
        })?;

        let cpu_profile_path = profile_dir.as_ref().join(format!(
            "{}-{}-cpuprofile.pb.gz",
            std::process::id(),
            unix_millis()
        ));
        let cpu_file = File::create(&cpu_profile_path).map_err(|err| {
            io::Error::new(
                err.kind(),
                format!("failed to create CPU profile file: {err}"),
            )
        })?;

        let cpu_profile_file = match cpu_file.try_clone() {
            Ok(cpu_profile_file) => cpu_profile_file,
            Err(err) => {
                drop(cpu_file);
                let _ = fs::remove_file(&cpu_profile_path);
                return Err(io::Error::new(
                    err.kind(),
                    format!("failed to start CPU profile: {err}"),
                ));
            }
        };
        let cpu_profile = match runtime_pprof::CpuProfile::start(cpu_profile_file) {
            Ok(cpu_profile) => cpu_profile,
            Err(err) => {
                drop(cpu_file);
                let _ = fs::remove_file(&cpu_profile_path);
                return Err(io::Error::new(
                    err.kind(),
                    format!("failed to start CPU profile: {err}"),
                ));
            }
        };

        *session = Some(ProfileSession {
            cpu_file_path: cpu_profile_path,
            mem_file_path: PathBuf::new(),
            cpu_file: Some(cpu_file),
            cpu_profile: Some(cpu_profile),
            log_writer: io::sink(),
            stopped: false,
        });
        Ok(())
    }

    // StopCPUProfile stops CPU profiling and returns the path to the profile file.
    pub fn stop_cpu_profile(&self) -> io::Result<PathBuf> {
        let mut session = self.session.lock().unwrap_or_else(|err| err.into_inner());

        let Some(mut current_session) = session.take() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "CPU profiling not in progress",
            ));
        };

        let file_path = current_session.cpu_file_path.clone();
        current_session.stop();

        Ok(file_path)
    }
}

impl<W: Write> Drop for ProfileSession<W> {
    fn drop(&mut self) {
        self.stop();
    }
}

// SaveHeapProfile saves a heap profile to the specified directory.
pub fn save_heap_profile(profile_dir: impl AsRef<Path>) -> io::Result<PathBuf> {
    fs::create_dir_all(&profile_dir).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("failed to create profile directory: {err}"),
        )
    })?;

    let heap_profile_path = profile_dir.as_ref().join(format!(
        "{}-{}-heapprofile.pb.gz",
        std::process::id(),
        unix_millis()
    ));
    let mut heap_file = File::create(&heap_profile_path).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("failed to create heap profile file: {err}"),
        )
    })?;

    run_gc();
    if let Err(err) =
        runtime_pprof::lookup(runtime_pprof::ProfileName::Heap).write_to(&mut heap_file, 0)
    {
        let _ = fs::remove_file(&heap_profile_path);
        return Err(io::Error::new(
            err.kind(),
            format!("failed to write heap profile: {err}"),
        ));
    }

    Ok(heap_profile_path)
}

// SaveAllocProfile saves an allocation profile to the specified directory.
pub fn save_alloc_profile(profile_dir: impl AsRef<Path>) -> io::Result<PathBuf> {
    fs::create_dir_all(&profile_dir).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("failed to create profile directory: {err}"),
        )
    })?;

    let alloc_profile_path = profile_dir.as_ref().join(format!(
        "{}-{}-allocprofile.pb.gz",
        std::process::id(),
        unix_millis()
    ));
    let mut alloc_file = File::create(&alloc_profile_path).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("failed to create alloc profile file: {err}"),
        )
    })?;

    if let Err(err) =
        runtime_pprof::lookup(runtime_pprof::ProfileName::Allocs).write_to(&mut alloc_file, 0)
    {
        let _ = fs::remove_file(&alloc_profile_path);
        return Err(io::Error::new(
            err.kind(),
            format!("failed to write alloc profile: {err}"),
        ));
    }

    Ok(alloc_profile_path)
}

// RunGC triggers garbage collection.
pub fn run_gc() {
    runtime_pprof::run_gc();
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
