use std::collections::HashSet;

use ts_lsproto as lsproto;

const EXCESSIVE_CHANGE_THRESHOLD: usize = 1000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum FileChangeKind {
    Open = 0,
    Close = 1,
    Change = 2,
    Save = 3,
    WatchCreate = 4,
    WatchChange = 5,
    WatchDelete = 6,
}

impl FileChangeKind {
    pub fn is_watch_kind(self) -> bool {
        self == FileChangeKind::WatchCreate
            || self == FileChangeKind::WatchChange
            || self == FileChangeKind::WatchDelete
    }
}

pub type DocumentUri = String;
pub type LanguageKind = String;
pub type TextDocumentContentChangePartialOrWholeDocument =
    lsproto::TextDocumentContentChangePartialOrWholeDocument;

pub struct FileChange {
    pub kind: FileChangeKind,
    pub uri: DocumentUri,
    pub version: i32,                // Only set for Open/Change
    pub content: String,             // Only set for Open
    pub language_kind: LanguageKind, // Only set for Open
    pub changes: Vec<TextDocumentContentChangePartialOrWholeDocument>, // Only set for Change
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileChangeSummary {
    // Only one file can be opened at a time per request
    pub opened: DocumentUri,
    // Reopened is set if a close and open occurred for the same file in a single batch of changes.
    pub reopened: DocumentUri,
    pub closed: HashSet<DocumentUri>,
    pub changed: HashSet<DocumentUri>,
    // Only set when file watching is enabled
    pub created: HashSet<DocumentUri>,
    // Only set when file watching is enabled
    pub deleted: HashSet<DocumentUri>,

    // IncludesWatchChangeOutsideNodeModules is true if the summary includes a create, change, or delete watch
    // event of a file outside a node_modules directory.
    pub includes_watch_change_outside_node_modules: bool,
    // InvalidateAll indicates that all cached file state should be discarded.
    pub invalidate_all: bool,
}

impl FileChangeSummary {
    pub fn is_empty(&self) -> bool {
        !self.invalidate_all
            && self.opened.is_empty()
            && self.reopened.is_empty()
            && self.closed.is_empty()
            && self.changed.is_empty()
            && self.created.is_empty()
            && self.deleted.is_empty()
    }

    pub fn has_excessive_watch_events(&self) -> bool {
        self.invalidate_all
            || self.created.len() + self.deleted.len() + self.changed.len()
                > EXCESSIVE_CHANGE_THRESHOLD
    }

    pub fn has_excessive_non_create_watch_events(&self) -> bool {
        self.invalidate_all || self.deleted.len() + self.changed.len() > EXCESSIVE_CHANGE_THRESHOLD
    }
}

// mergeFileChangeSummary merges src into dst, combining their change sets.
pub fn merge_file_change_summary(dst: &mut FileChangeSummary, src: FileChangeSummary) {
    if src.is_empty() {
        return;
    }
    if src.invalidate_all {
        dst.invalidate_all = true;
    }
    for uri in src.changed {
        dst.changed.insert(uri);
    }
    for uri in src.created {
        dst.created.insert(uri);
    }
    for uri in src.deleted {
        dst.deleted.insert(uri);
    }
    if src.includes_watch_change_outside_node_modules {
        dst.includes_watch_change_outside_node_modules = true;
    }
}
