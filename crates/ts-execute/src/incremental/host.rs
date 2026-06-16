use std::time::SystemTime;

pub use ts_compiler::CompilerHost;
pub use ts_vfs::Fs as FileSystem;

pub trait Host: Send + Sync {
    fn get_m_time(&self, file_name: &str) -> SystemTime;
    fn set_m_time(&self, file_name: &str, m_time: SystemTime) -> Result<(), String>;
}

struct host<H> {
    host: H,
}

impl<H> Host for host<H>
where
    H: CompilerHost,
{
    fn get_m_time(&self, file_name: &str) -> SystemTime {
        get_mtime(&self.host, file_name)
    }

    fn set_m_time(&self, file_name: &str, m_time: SystemTime) -> Result<(), String> {
        self.host
            .fs()
            .chtimes(file_name, SystemTime::UNIX_EPOCH, m_time)
            .map_err(|err| err.to_string())
    }
}

pub fn create_host<H>(compiler_host: H) -> Box<dyn Host>
where
    H: CompilerHost + 'static,
{
    Box::new(host {
        host: compiler_host,
    })
}

pub fn get_mtime(host: &impl CompilerHost, file_name: &str) -> SystemTime {
    if let Ok(stat) = host.fs().stat(file_name) {
        return stat.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    }
    SystemTime::UNIX_EPOCH
}
