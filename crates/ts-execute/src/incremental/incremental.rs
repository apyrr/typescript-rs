use ts_tsoptions as tsoptions;

use super::{BuildInfo, Program, build_info_to_snapshot, host::CompilerHost};

pub trait BuildInfoReader {
    fn read_build_info(&self, config: &tsoptions::ParsedCommandLine) -> Option<BuildInfo>;
}

impl<T: BuildInfoReader + ?Sized> BuildInfoReader for std::sync::Arc<T> {
    fn read_build_info(&self, config: &tsoptions::ParsedCommandLine) -> Option<BuildInfo> {
        (**self).read_build_info(config)
    }
}

struct BuildInfoReaderImpl<H> {
    host: H,
}

impl<H> BuildInfoReader for BuildInfoReaderImpl<H>
where
    H: CompilerHost,
{
    fn read_build_info(&self, config: &tsoptions::ParsedCommandLine) -> Option<BuildInfo> {
        let build_info_file_name = config.get_build_info_file_name();
        if build_info_file_name.is_empty() {
            return None;
        }

        // Read build info file
        let (data, ok) = self.host.fs().read_file(&build_info_file_name);
        if !ok {
            return None;
        }
        serde_json::from_str(&data).ok()
    }
}

pub fn new_build_info_reader<H>(host: H) -> impl BuildInfoReader
where
    H: CompilerHost,
{
    BuildInfoReaderImpl { host }
}

pub fn read_build_info_program(
    config: &tsoptions::ParsedCommandLine,
    reader: &dyn BuildInfoReader,
    host: &dyn CompilerHost,
) -> Option<Program> {
    // Read buildInfo file
    let build_info = reader.read_build_info(config)?;
    if !build_info.is_valid_version() || !build_info.is_incremental() {
        return None;
    }

    // Convert to information that can be used to create incremental program
    let incremental_program = Program {
        snapshot: build_info_to_snapshot(&build_info, config, host),
        program: None,
        host: None,
        testing_data: None,
    };
    Some(incremental_program)
}
