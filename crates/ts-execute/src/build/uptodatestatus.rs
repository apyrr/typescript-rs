use std::time::SystemTime;

pub type UpToDateStatusType = u16;

// Errors:

// config file was not found
pub const UP_TO_DATE_STATUS_TYPE_CONFIG_FILE_NOT_FOUND: UpToDateStatusType = 0;
// found errors during build
pub const UP_TO_DATE_STATUS_TYPE_BUILD_ERRORS: UpToDateStatusType = 1;
// did not build because upstream project has errors - and we have option to stop build on upstream errors
pub const UP_TO_DATE_STATUS_TYPE_UPSTREAM_ERRORS: UpToDateStatusType = 2;

// Its all good, no work to do
pub const UP_TO_DATE_STATUS_TYPE_UP_TO_DATE: UpToDateStatusType = 3;

// Pseudo-builds - touch timestamps, no actual build:

// The project appears out of date because its upstream inputs are newer than its outputs,
// but all of its outputs are actually newer than the previous identical outputs of its (.d.ts) inputs.
// This means we can Pseudo-build (just touch timestamps), as if we had actually built this project.
pub const UP_TO_DATE_STATUS_TYPE_UP_TO_DATE_WITH_UPSTREAM_TYPES: UpToDateStatusType = 4;
// The project appears up to date and even though input file changed, its text didnt so just need to update timestamps
pub const UP_TO_DATE_STATUS_TYPE_UP_TO_DATE_WITH_INPUT_FILE_TEXT: UpToDateStatusType = 5;

// Needs build:

// input file is missing
pub const UP_TO_DATE_STATUS_TYPE_INPUT_FILE_MISSING: UpToDateStatusType = 6;
// output file is missing
pub const UP_TO_DATE_STATUS_TYPE_OUTPUT_MISSING: UpToDateStatusType = 7;
// input file is newer than output file
pub const UP_TO_DATE_STATUS_TYPE_INPUT_FILE_NEWER: UpToDateStatusType = 8;
// build info is out of date as we need to emit some files
pub const UP_TO_DATE_STATUS_TYPE_OUT_OF_DATE_BUILD_INFO_WITH_PENDING_EMIT: UpToDateStatusType = 9;
// build info indicates that project has errors and they need to be reported
pub const UP_TO_DATE_STATUS_TYPE_OUT_OF_DATE_BUILD_INFO_WITH_ERRORS: UpToDateStatusType = 10;
// build info options indicate there is work to do based on changes in options
pub const UP_TO_DATE_STATUS_TYPE_OUT_OF_DATE_OPTIONS: UpToDateStatusType = 11;
// file was root when built but not any more
pub const UP_TO_DATE_STATUS_TYPE_OUT_OF_DATE_ROOTS: UpToDateStatusType = 12;
// buildInfo.version mismatch with current ts version
pub const UP_TO_DATE_STATUS_TYPE_TS_VERSION_OUTPUT_OF_DATE: UpToDateStatusType = 13;
// build because --force was specified
pub const UP_TO_DATE_STATUS_TYPE_FORCE_BUILD: UpToDateStatusType = 14;

// solution file
pub const UP_TO_DATE_STATUS_TYPE_SOLUTION: UpToDateStatusType = 15;

#[derive(Clone, Default)]
pub struct InputOutputName {
    pub input: String,
    pub output: String,
}

#[derive(Clone)]
pub struct FileAndTime {
    pub file: String,
    pub time: SystemTime,
}

impl Default for FileAndTime {
    fn default() -> Self {
        Self {
            file: String::new(),
            time: SystemTime::UNIX_EPOCH,
        }
    }
}

#[derive(Clone)]
pub struct InputOutputFileAndTime {
    pub input: FileAndTime,
    pub output: FileAndTime,
    pub build_info: String,
}

#[derive(Clone, Default)]
pub struct UpstreamErrors {
    pub r#ref: String,
    pub ref_has_upstream_errors: bool,
}

#[derive(Clone)]
pub enum StatusData {
    None,
    String(String),
    InputOutputName(InputOutputName),
    InputOutputFileAndTime(InputOutputFileAndTime),
    UpstreamErrors(UpstreamErrors),
}

#[derive(Clone)]
pub struct UpToDateStatus {
    pub kind: UpToDateStatusType,
    pub data: StatusData,
}

impl UpToDateStatus {
    pub fn new(kind: UpToDateStatusType) -> Self {
        Self {
            kind,
            data: StatusData::None,
        }
    }

    pub fn with_string(kind: UpToDateStatusType, data: String) -> Self {
        Self {
            kind,
            data: StatusData::String(data),
        }
    }

    pub fn is_error(&self) -> bool {
        match self.kind {
            UP_TO_DATE_STATUS_TYPE_CONFIG_FILE_NOT_FOUND
            | UP_TO_DATE_STATUS_TYPE_BUILD_ERRORS
            | UP_TO_DATE_STATUS_TYPE_UPSTREAM_ERRORS => true,
            _ => false,
        }
    }

    pub fn is_pseudo_build(&self) -> bool {
        match self.kind {
            UP_TO_DATE_STATUS_TYPE_UP_TO_DATE_WITH_UPSTREAM_TYPES
            | UP_TO_DATE_STATUS_TYPE_UP_TO_DATE_WITH_INPUT_FILE_TEXT => true,
            _ => false,
        }
    }

    pub fn input_output_file_and_time(&self) -> Option<&InputOutputFileAndTime> {
        match &self.data {
            StatusData::InputOutputFileAndTime(data) => Some(data),
            _ => None,
        }
    }

    pub fn input_output_name(&self) -> Option<&InputOutputName> {
        match &self.data {
            StatusData::InputOutputName(data) => Some(data),
            _ => None,
        }
    }

    pub fn oldest_output_file_name(&self) -> String {
        if !self.is_pseudo_build() && self.kind != UP_TO_DATE_STATUS_TYPE_UP_TO_DATE {
            panic!("only valid for up to date status of pseudo-build or up to date")
        }

        if let Some(input_output_file_and_time) = self.input_output_file_and_time() {
            return input_output_file_and_time.output.file.clone();
        }
        if let Some(input_output_name) = self.input_output_name() {
            return input_output_name.output.clone();
        }
        match &self.data {
            StatusData::String(data) => data.clone(),
            _ => panic!("string status data expected"),
        }
    }

    pub fn upstream_errors(&self) -> &UpstreamErrors {
        match &self.data {
            StatusData::UpstreamErrors(data) => data,
            _ => panic!("upstream errors status data expected"),
        }
    }
}
