use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::Tristate;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WatchOptions {
    #[serde(rename = "watchInterval")]
    pub interval: Option<i32>,
    #[serde(rename = "watchFile")]
    pub file_kind: WatchFileKind,
    #[serde(rename = "watchDirectory")]
    pub directory_kind: WatchDirectoryKind,
    #[serde(rename = "fallbackPolling")]
    pub fallback_polling: PollingKind,
    #[serde(rename = "synchronousWatchDirectory")]
    pub sync_watch_dir: Tristate,
    #[serde(rename = "excludeDirectories")]
    pub exclude_dir: Option<Vec<String>>,
    #[serde(rename = "excludeFiles")]
    pub exclude_files: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WatchFileKind(pub i32);

impl WatchFileKind {
    #[allow(non_upper_case_globals)]
    pub const None: WatchFileKind = WatchFileKind(0);
    #[allow(non_upper_case_globals)]
    pub const FixedPollingInterval: WatchFileKind = WatchFileKind(1);
    #[allow(non_upper_case_globals)]
    pub const PriorityPollingInterval: WatchFileKind = WatchFileKind(2);
    #[allow(non_upper_case_globals)]
    pub const DynamicPriorityPolling: WatchFileKind = WatchFileKind(3);
    #[allow(non_upper_case_globals)]
    pub const FixedChunkSizePolling: WatchFileKind = WatchFileKind(4);
    #[allow(non_upper_case_globals)]
    pub const UseFsEvents: WatchFileKind = WatchFileKind(5);
    #[allow(non_upper_case_globals)]
    pub const UseFsEventsOnParentDirectory: WatchFileKind = WatchFileKind(6);
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WatchDirectoryKind(pub i32);

impl WatchDirectoryKind {
    #[allow(non_upper_case_globals)]
    pub const None: WatchDirectoryKind = WatchDirectoryKind(0);
    #[allow(non_upper_case_globals)]
    pub const UseFsEvents: WatchDirectoryKind = WatchDirectoryKind(1);
    #[allow(non_upper_case_globals)]
    pub const FixedPollingInterval: WatchDirectoryKind = WatchDirectoryKind(2);
    #[allow(non_upper_case_globals)]
    pub const DynamicPriorityPolling: WatchDirectoryKind = WatchDirectoryKind(3);
    #[allow(non_upper_case_globals)]
    pub const FixedChunkSizePolling: WatchDirectoryKind = WatchDirectoryKind(4);
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PollingKind(pub i32);

impl PollingKind {
    #[allow(non_upper_case_globals)]
    pub const None: PollingKind = PollingKind(0);
    #[allow(non_upper_case_globals)]
    pub const FixedInterval: PollingKind = PollingKind(1);
    #[allow(non_upper_case_globals)]
    pub const PriorityInterval: PollingKind = PollingKind(2);
    #[allow(non_upper_case_globals)]
    pub const DynamicPriority: PollingKind = PollingKind(3);
    #[allow(non_upper_case_globals)]
    pub const FixedChunkSize: PollingKind = PollingKind(4);
}

impl WatchOptions {
    pub fn watch_interval(&self) -> Duration {
        let watch_interval = self.interval.unwrap_or(2000);
        Duration::from_millis(watch_interval as u64)
    }
}

pub fn watch_interval(options: Option<&WatchOptions>) -> Duration {
    options
        .map(WatchOptions::watch_interval)
        .unwrap_or_else(|| Duration::from_millis(2000))
}
