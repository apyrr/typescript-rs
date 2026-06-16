#![forbid(unsafe_code)]
mod arena;
mod bfs;
#[cfg(test)]
mod bfs_test;
mod binarysearch;
mod buildoptions;
mod compileroptions;
pub mod context;
mod core;
mod languagevariant;
mod languagevariant_stringer_generated;
mod linkstore;
#[cfg(test)]
mod linkstore_test;
mod modulekind;
mod modulekind_stringer_generated;
mod nodemodules;
mod parsedoptions;
mod pattern;
mod projectreference;
mod scriptkind;
mod scriptkind_stringer_generated;
mod scripttarget;
mod scripttarget_stringer_generated;
mod semaphore;
mod stack;
mod text;
mod textchange;
mod tristate;
mod tristate_stringer_generated;
mod typeacquisition;
mod version;
mod watchoptions;
mod workgroup;

pub use arena::Arena;
pub use bfs::{
    BreadthFirstSearchLevel, BreadthFirstSearchOptions, BreadthFirstSearchResult,
    breadth_first_search_parallel, breadth_first_search_parallel_ex,
};
pub use binarysearch::binary_search_unique_func;
pub use buildoptions::BuildOptions;
pub use compileroptions::{
    CompilerOptions, JsxEmit, ModuleDetectionKind, ModuleResolutionKind, NewLineKind,
    empty_compiler_options, get_new_line_kind, module_kind_to_module_resolution_kind,
};
pub use context::{
    CancelFunc, Context, Tick, Timer, after_func, get_request_id, new_ticker, sleep_or_done,
    with_cancel, with_request_id,
};
pub use core::*;
pub use languagevariant::LanguageVariant;
pub use linkstore::{
    IntoLinkKey, LinkHandle, LinkStore, LinkStoreStatsSnapshot, link_store_stats_available,
    link_store_stats_snapshot, reset_link_store_stats, set_link_store_stats_enabled,
};
pub use modulekind::{
    ModuleKind, RESOLUTION_MODE_COMMON_JS, RESOLUTION_MODE_ESM, RESOLUTION_MODE_NONE,
    ResolutionMode,
};
pub use nodemodules::{
    EXCLUSIVELY_PREFIXED_NODE_CORE_MODULES, UNPREFIXED_NODE_CORE_MODULES, node_core_modules,
    non_relative_module_name_for_typing_cache,
};
pub use parsedoptions::{ParsedOptions, WatchOptions};
pub use pattern::{Pattern, find_best_pattern_match, try_parse_pattern};
pub use projectreference::{
    ProjectReference, resolve_config_file_name_of_project_reference, resolve_project_reference_path,
};
pub use scriptkind::ScriptKind;
pub use scripttarget::{SCRIPT_TARGET_LATEST, SCRIPT_TARGET_LATEST_STANDARD, ScriptTarget};
pub use semaphore::{LimitedSemaphore, Semaphore, UnlimitedSemaphore, new_limited_semaphore};
pub use stack::Stack;
pub use text::{TextPos, TextRange, compare_text_ranges, new_text_range, undefined_text_range};
pub use textchange::{TextChange, apply_bulk_edits};
pub use tristate::{TS_FALSE, TS_TRUE, TS_UNKNOWN, Tristate, bool_to_tristate};
pub use ts_collections::Set;
pub use typeacquisition::{TypeAcquisition, type_acquisition_equals};
pub use version::{version, version_major_minor};
pub use watchoptions::{
    PollingKind, WatchDirectoryKind, WatchFileKind, WatchOptions as CoreWatchOptions,
    watch_interval,
};
pub use workgroup::{ThrottleGroup, WorkGroup, new_throttle_group, new_work_group};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub message: String,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

#[allow(non_snake_case)]
pub fn TextPos(value: i32) -> TextPos {
    value
}

pub const LANGUAGE_VARIANT_STANDARD: LanguageVariant = LanguageVariant::Standard;
pub const LANGUAGE_VARIANT_JSX: LanguageVariant = LanguageVariant::JSX;

pub const SCRIPT_KIND_UNKNOWN: ScriptKind = ScriptKind::Unknown;
pub const SCRIPT_KIND_JS: ScriptKind = ScriptKind::JS;
pub const SCRIPT_KIND_JSX: ScriptKind = ScriptKind::JSX;
pub const SCRIPT_KIND_TS: ScriptKind = ScriptKind::TS;
pub const SCRIPT_KIND_TSX: ScriptKind = ScriptKind::TSX;
pub const SCRIPT_KIND_EXTERNAL: ScriptKind = ScriptKind::External;
pub const SCRIPT_KIND_JSON: ScriptKind = ScriptKind::JSON;
pub const SCRIPT_KIND_DEFERRED: ScriptKind = ScriptKind::Deferred;

pub const MODULE_DETECTION_KIND_NONE: ModuleDetectionKind = ModuleDetectionKind::None;
pub const MODULE_DETECTION_KIND_AUTO: ModuleDetectionKind = ModuleDetectionKind::Auto;
pub const MODULE_DETECTION_KIND_LEGACY: ModuleDetectionKind = ModuleDetectionKind::Legacy;
pub const MODULE_DETECTION_KIND_FORCE: ModuleDetectionKind = ModuleDetectionKind::Force;

pub const MODULE_KIND_NONE: ModuleKind = ModuleKind::None;
pub const MODULE_KIND_COMMON_JS: ModuleKind = ModuleKind::CommonJS;
pub const MODULE_KIND_AMD: ModuleKind = ModuleKind::AMD;
pub const MODULE_KIND_UMD: ModuleKind = ModuleKind::UMD;
pub const MODULE_KIND_SYSTEM: ModuleKind = ModuleKind::System;
pub const MODULE_KIND_ES2015: ModuleKind = ModuleKind::ES2015;
pub const MODULE_KIND_ES2020: ModuleKind = ModuleKind::ES2020;
pub const MODULE_KIND_ES2022: ModuleKind = ModuleKind::ES2022;
pub const MODULE_KIND_ES_NEXT: ModuleKind = ModuleKind::ESNext;
pub const MODULE_KIND_NODE16: ModuleKind = ModuleKind::Node16;
pub const MODULE_KIND_NODE18: ModuleKind = ModuleKind::Node18;
pub const MODULE_KIND_NODE20: ModuleKind = ModuleKind::Node20;
pub const MODULE_KIND_NODE_NEXT: ModuleKind = ModuleKind::NodeNext;
pub const MODULE_KIND_PRESERVE: ModuleKind = ModuleKind::Preserve;

pub const MODULE_RESOLUTION_KIND_UNKNOWN: ModuleResolutionKind = ModuleResolutionKind::Unknown;
pub const MODULE_RESOLUTION_KIND_CLASSIC: ModuleResolutionKind = ModuleResolutionKind::Classic;
pub const MODULE_RESOLUTION_KIND_NODE10: ModuleResolutionKind = ModuleResolutionKind::Node10;
pub const MODULE_RESOLUTION_KIND_NODE16: ModuleResolutionKind = ModuleResolutionKind::Node16;
pub const MODULE_RESOLUTION_KIND_NODE_NEXT: ModuleResolutionKind = ModuleResolutionKind::NodeNext;
pub const MODULE_RESOLUTION_KIND_BUNDLER: ModuleResolutionKind = ModuleResolutionKind::Bundler;

pub const NEW_LINE_KIND_NONE: NewLineKind = NewLineKind::None;
pub const NEW_LINE_KIND_CRLF: NewLineKind = NewLineKind::CRLF;
pub const NEW_LINE_KIND_LF: NewLineKind = NewLineKind::LF;

pub const SCRIPT_TARGET_NONE: ScriptTarget = ScriptTarget::None;
pub const SCRIPT_TARGET_ES5: ScriptTarget = ScriptTarget::ES5;
pub const SCRIPT_TARGET_ES2015: ScriptTarget = ScriptTarget::ES2015;
pub const SCRIPT_TARGET_ES2016: ScriptTarget = ScriptTarget::ES2016;
pub const SCRIPT_TARGET_ES2017: ScriptTarget = ScriptTarget::ES2017;
pub const SCRIPT_TARGET_ES2018: ScriptTarget = ScriptTarget::ES2018;
pub const SCRIPT_TARGET_ES2019: ScriptTarget = ScriptTarget::ES2019;
pub const SCRIPT_TARGET_ES2020: ScriptTarget = ScriptTarget::ES2020;
pub const SCRIPT_TARGET_ES2021: ScriptTarget = ScriptTarget::ES2021;
pub const SCRIPT_TARGET_ES2022: ScriptTarget = ScriptTarget::ES2022;
pub const SCRIPT_TARGET_ES2023: ScriptTarget = ScriptTarget::ES2023;
pub const SCRIPT_TARGET_ES2024: ScriptTarget = ScriptTarget::ES2024;
pub const SCRIPT_TARGET_ES2025: ScriptTarget = ScriptTarget::ES2025;
pub const SCRIPT_TARGET_ES_NEXT: ScriptTarget = ScriptTarget::ESNext;
pub const SCRIPT_TARGET_JSON: ScriptTarget = ScriptTarget::JSON;

pub const JSX_EMIT_NONE: JsxEmit = JsxEmit::None;
pub const JSX_EMIT_PRESERVE: JsxEmit = JsxEmit::Preserve;
pub const JSX_EMIT_REACT_NATIVE: JsxEmit = JsxEmit::ReactNative;
pub const JSX_EMIT_REACT: JsxEmit = JsxEmit::React;
pub const JSX_EMIT_REACT_JSX: JsxEmit = JsxEmit::ReactJSX;
pub const JSX_EMIT_REACT_JSX_DEV: JsxEmit = JsxEmit::ReactJSXDev;

pub const WATCH_FILE_KIND_NONE: WatchFileKind = WatchFileKind::None;
pub const WATCH_FILE_KIND_FIXED_POLLING_INTERVAL: WatchFileKind =
    WatchFileKind::FixedPollingInterval;
pub const WATCH_FILE_KIND_PRIORITY_POLLING_INTERVAL: WatchFileKind =
    WatchFileKind::PriorityPollingInterval;
pub const WATCH_FILE_KIND_DYNAMIC_PRIORITY_POLLING: WatchFileKind =
    WatchFileKind::DynamicPriorityPolling;
pub const WATCH_FILE_KIND_FIXED_CHUNK_SIZE_POLLING: WatchFileKind =
    WatchFileKind::FixedChunkSizePolling;
pub const WATCH_FILE_KIND_USE_FS_EVENTS: WatchFileKind = WatchFileKind::UseFsEvents;
pub const WATCH_FILE_KIND_USE_FS_EVENTS_ON_PARENT_DIRECTORY: WatchFileKind =
    WatchFileKind::UseFsEventsOnParentDirectory;

pub const WATCH_DIRECTORY_KIND_NONE: WatchDirectoryKind = WatchDirectoryKind::None;
pub const WATCH_DIRECTORY_KIND_USE_FS_EVENTS: WatchDirectoryKind = WatchDirectoryKind::UseFsEvents;
pub const WATCH_DIRECTORY_KIND_FIXED_POLLING_INTERVAL: WatchDirectoryKind =
    WatchDirectoryKind::FixedPollingInterval;
pub const WATCH_DIRECTORY_KIND_DYNAMIC_PRIORITY_POLLING: WatchDirectoryKind =
    WatchDirectoryKind::DynamicPriorityPolling;
pub const WATCH_DIRECTORY_KIND_FIXED_CHUNK_SIZE_POLLING: WatchDirectoryKind =
    WatchDirectoryKind::FixedChunkSizePolling;

pub const POLLING_KIND_NONE: PollingKind = PollingKind::None;
pub const POLLING_KIND_FIXED_INTERVAL: PollingKind = PollingKind::FixedInterval;
pub const POLLING_KIND_PRIORITY_INTERVAL: PollingKind = PollingKind::PriorityInterval;
pub const POLLING_KIND_DYNAMIC_PRIORITY: PollingKind = PollingKind::DynamicPriority;
pub const POLLING_KIND_FIXED_CHUNK_SIZE: PollingKind = PollingKind::FixedChunkSize;

#[allow(non_upper_case_globals)]
pub const TSUnknown: Tristate = TS_UNKNOWN;
#[allow(non_upper_case_globals)]
pub const TSFalse: Tristate = TS_FALSE;
#[allow(non_upper_case_globals)]
pub const TSTrue: Tristate = TS_TRUE;
