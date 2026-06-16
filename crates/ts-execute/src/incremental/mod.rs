mod affectedfileshandler;
#[path = "buildInfo.rs"]
mod build_info;
mod buildinfotosnapshot;
mod emitfileshandler;
mod host;
mod incremental;
mod program;
mod programtosnapshot;
mod referencemap;
mod snapshot;
mod snapshottobuildinfo;

pub use build_info::{
    BuildInfo, BuildInfoDiagnostic, BuildInfoDiagnosticsOfFile, BuildInfoEmitSignature,
    BuildInfoFileId, BuildInfoFileIdListId, BuildInfoFileInfo, BuildInfoFilePendingEmit,
    BuildInfoReferenceMapEntry, BuildInfoRepopulateInfo, BuildInfoResolvedRoot, BuildInfoRoot,
    BuildInfoRootInfoReader, BuildInfoSemanticDiagnostic,
};
pub use host::{Host, create_host, get_mtime, get_mtime as get_m_time};
pub use incremental::{BuildInfoReader, new_build_info_reader, read_build_info_program};
pub(crate) use program::MaybeProgramExt;
pub use program::{
    Program, SIGNATURE_UPDATE_KIND_COMPUTED_DTS, SIGNATURE_UPDATE_KIND_STORED_AT_EMIT,
    SIGNATURE_UPDATE_KIND_USED_VERSION, SignatureUpdateKind, TestingData, new_program,
};
pub use snapshot::{
    FILE_EMIT_KIND_ALL, FILE_EMIT_KIND_ALL_DTS, FILE_EMIT_KIND_ALL_DTS_EMIT, FILE_EMIT_KIND_ALL_JS,
    FILE_EMIT_KIND_DTS, FILE_EMIT_KIND_DTS_EMIT, FILE_EMIT_KIND_DTS_ERRORS, FILE_EMIT_KIND_DTS_MAP,
    FILE_EMIT_KIND_JS, FILE_EMIT_KIND_JS_INLINE_MAP, FILE_EMIT_KIND_JS_MAP, FILE_EMIT_KIND_NONE,
    FileEmitKind, FileInfo, Snapshot, compute_hash, get_file_emit_kind,
};

pub(crate) use affectedfileshandler::*;
pub(crate) use build_info::{is_default, new_build_info_file_info};
pub(crate) use buildinfotosnapshot::*;
pub(crate) use emitfileshandler::*;
pub(crate) use snapshot::{
    BuildInfoDiagnosticWithFileName, DiagnosticsOrBuildInfoDiagnosticsWithFileName,
    get_pending_emit_kind_with_options, repopulate_diagnostic_chain,
};
pub(crate) use snapshottobuildinfo::*;
