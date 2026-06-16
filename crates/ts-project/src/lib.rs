#![forbid(unsafe_code)]

pub(crate) use ts_diagnostics as diagnostics;
pub(crate) use ts_project_support::{dirty, logging};
pub(crate) use ts_tsoptions as tsoptions;

mod api;
#[expect(
    dead_code,
    reason = "ported ATA project support is ahead of current callers"
)]
mod ata;
mod autoimport;
mod background;
mod checkerpool;
mod client;
mod compilerhost;
mod configfileregistry;
#[expect(
    dead_code,
    reason = "ported config registry builder APIs are ahead of current callers"
)]
mod configfileregistrybuilder;
mod extendedconfigcache;
mod filechange;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "ported overlay filesystem support is ahead of current callers"
    )
)]
mod overlayfs;
#[expect(
    dead_code,
    reason = "ported owner cache support is ahead of current callers"
)]
mod ownercache;
mod parsecache;
mod programcounter;
#[expect(
    dead_code,
    reason = "ported project state APIs are ahead of current callers"
)]
mod project;
mod project_stringer_generated;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "ported project collection APIs are ahead of current callers"
    )
)]
mod projectcollection;
mod projectcollectionbuilder;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "ported ref-count cache APIs are ahead of current callers"
    )
)]
mod refcountcache;
#[expect(dead_code, reason = "ported session APIs are ahead of current callers")]
mod session;
#[expect(
    dead_code,
    reason = "ported snapshot APIs are ahead of current callers"
)]
mod snapshot;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "ported snapshot filesystem APIs are ahead of current callers"
    )
)]
mod snapshotfs;
mod watch;

#[cfg(test)]
mod bulkcache_test;
#[cfg(test)]
mod configfilechanges_test;
#[cfg(test)]
mod customconfigfilename_test;
#[cfg(test)]
mod extendedconfigcache_test;
#[cfg(test)]
mod overlayfs_test;
#[cfg(test)]
mod project_test;
#[cfg(test)]
mod projectcollectionbuilder_test;
#[cfg(test)]
mod projectcollectiondefaultproject_test;
#[cfg(test)]
mod projectlifetime_test;
#[cfg(test)]
mod projectreferencesprogram_test;
#[cfg(test)]
#[expect(
    dead_code,
    reason = "shared project test utilities are not used by every test target"
)]
mod projecttestutil;
#[cfg(test)]
mod refcountcache_test;
#[cfg(test)]
mod session_test;
#[cfg(test)]
mod snapshot_test;
#[cfg(test)]
mod snapshotfs_test;
#[cfg(test)]
#[expect(
    dead_code,
    reason = "generated test entry points call this in harness builds"
)]
mod testmain_test;
#[cfg(test)]
mod untitled_test;
#[cfg(test)]
mod watch_test;
#[cfg(test)]
mod watchtimeout_test;

pub use ata::NpmExecutor;
pub use client::{
    Client, ClientHandle, Context, DiagnosticsMessage, FileSystemWatcher, PublishDiagnosticsParams,
    TelemetryEvent, WatcherID,
};
pub use configfileregistry::{ConfigFileRegistry, TestConfigEntry, TestConfigFileNamesEntry};
pub use filechange::{
    DocumentUri, FileChangeSummary, LanguageKind, TextDocumentContentChangePartialOrWholeDocument,
};
pub use parsecache::ParseCache;
pub use project::{Kind, Project, ProjectInfo};
pub use session::{Session, SessionInit, SessionOptions, new_session};
pub use snapshot::{ProjectTreeRequest, SnapshotHandle};
pub use ts_locale::Locale;
pub use ts_project_support::logging::{
    LogCollector, LogTree, Logger, new_log_tree, new_test_logger,
};

pub(crate) use autoimport::new_auto_import_registry_clone_host;
pub(crate) use checkerpool::{CheckerPoolHandle, new_checker_pool};
pub(crate) use compilerhost::{CompilerHost, new_compiler_host, new_compiler_host_handle};
pub(crate) use configfileregistrybuilder::{
    ChangeFileResult, ConfigFileRegistryBuilder, new_config_file_registry_builder,
};
pub(crate) use extendedconfigcache::{
    ExtendedConfigCache, ExtendedConfigParseArgs, new_extended_config_cache,
    parse_extended_config_cache_entry,
};
pub(crate) use filechange::{FileChange, FileChangeKind, merge_file_change_summary};
pub(crate) use overlayfs::{FileHandle, Overlay, OverlayFs, new_disk_file, new_overlay_fs};
pub(crate) use ownercache::{OwnerCache, new_owner_cache};
pub(crate) use parsecache::{ParseCacheKey, new_parse_cache, new_parse_cache_key};
pub(crate) use programcounter::ProgramCounter;
pub(crate) use project::{
    HR, INFERRED_PROJECT_NAME, PendingReload, ProgramUpdateKind, new_configured_project,
    new_inferred_project,
};
pub(crate) use projectcollection::{
    ProjectCollection, find_default_configured_project_from_program_inclusion,
};
pub(crate) use projectcollectionbuilder::{
    ProjectCollectionBuilder, ProjectLoadKind, new_project_collection_builder,
};
pub(crate) use refcountcache::{RefCountCache, RefCountCacheOptions, new_ref_count_cache};
pub(crate) use session::UpdateReason;
pub(crate) use snapshot::{
    ApiSnapshotRequest, AtaStateChange, ResourceRequest, Snapshot, SnapshotChange, new_snapshot,
};
pub(crate) use snapshotfs::{
    FileHandleRef, FileSource, SnapshotFs, SnapshotFsBuilder, SourceFs, new_snapshot_fs_builder,
    new_source_fs,
};
pub(crate) use watch::{
    MIN_WATCH_LOCATION_DEPTH, PatternsAndIgnored, WatchRegistry, WatchedFiles, WatcherId,
    create_resolution_lookup_glob_mapper, get_recursive_glob_pattern, get_typings_locations_globs,
    new_watch_registry, new_watched_files,
};
