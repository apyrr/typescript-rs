use std::{
    collections::HashMap,
    sync::{Arc, OnceLock, RwLock},
};

use ts_core as core;
use ts_ls as lsconv;
use ts_lsproto::{self as lsproto, DocumentUriExt};
use ts_sourcemap as sourcemap;
use ts_tspath as tspath;
use ts_vfs as vfs;
use xxhash_rust::xxh3;

use crate::{FileChange, FileChangeKind, FileChangeSummary, LanguageKind};

pub trait FileContent {
    fn content(&self) -> String;
    fn hash(&self) -> u128;
}

pub trait FileHandle: FileContent {
    fn file_name(&self) -> String;
    fn version(&self) -> i32;
    fn matches_disk_text(&self) -> bool;
    fn is_overlay(&self) -> bool;
    fn lsp_line_map(&self) -> &lsconv::LspLineMap;
    fn ecma_line_info(&self) -> &sourcemap::ECMALineInfo;
    fn kind(&self) -> core::ScriptKind;
}

pub struct FileBase {
    pub file_name: String,
    pub content: String,
    pub hash: u128,

    line_map: OnceLock<lsconv::LspLineMap>,
    line_info: OnceLock<sourcemap::ECMALineInfo>,
}

impl FileBase {
    pub fn file_name(&self) -> String {
        self.file_name.clone()
    }

    pub fn hash(&self) -> u128 {
        self.hash
    }

    pub fn content(&self) -> String {
        self.content.clone()
    }

    pub fn lsp_line_map(&self) -> &lsconv::LspLineMap {
        self.line_map
            .get_or_init(|| lsconv::compute_lsp_line_starts(&self.content))
    }

    pub fn ecma_line_info(&self) -> &sourcemap::ECMALineInfo {
        self.line_info.get_or_init(|| {
            let line_starts = core::compute_ecma_line_starts(&self.content);
            sourcemap::create_ecma_line_info(self.content.clone(), line_starts)
        })
    }
}

pub struct DiskFile {
    pub file_base: FileBase,
    pub needs_reload: bool,
    pub realpath_path: tspath::Path,
}

pub fn new_disk_file(file_name: String, content: String) -> DiskFile {
    DiskFile {
        file_base: FileBase {
            file_name,
            content: content.clone(),
            hash: xxh3::xxh3_128(content.as_bytes()),
            line_map: OnceLock::new(),
            line_info: OnceLock::new(),
        },
        needs_reload: false,
        realpath_path: tspath::Path::default(),
    }
}

impl FileContent for DiskFile {
    fn content(&self) -> String {
        self.file_base.content()
    }

    fn hash(&self) -> u128 {
        self.file_base.hash()
    }
}

impl FileHandle for DiskFile {
    fn file_name(&self) -> String {
        self.file_base.file_name()
    }

    fn version(&self) -> i32 {
        0
    }

    fn matches_disk_text(&self) -> bool {
        !self.needs_reload
    }

    fn is_overlay(&self) -> bool {
        false
    }

    fn lsp_line_map(&self) -> &lsconv::LspLineMap {
        self.file_base.lsp_line_map()
    }

    fn ecma_line_info(&self) -> &sourcemap::ECMALineInfo {
        self.file_base.ecma_line_info()
    }

    fn kind(&self) -> core::ScriptKind {
        core::get_script_kind_from_file_name(&self.file_base.file_name)
    }
}

impl Clone for DiskFile {
    fn clone(&self) -> Self {
        DiskFile {
            realpath_path: self.realpath_path.clone(),
            file_base: FileBase {
                file_name: self.file_base.file_name.clone(),
                content: self.file_base.content.clone(),
                hash: self.file_base.hash,
                line_map: OnceLock::new(),
                line_info: OnceLock::new(),
            },
            needs_reload: self.needs_reload,
        }
    }
}

pub struct Overlay {
    pub file_base: FileBase,
    pub version: i32,
    pub kind: core::ScriptKind,
    pub matches_disk_text: bool,
}

impl Clone for Overlay {
    fn clone(&self) -> Self {
        Overlay {
            file_base: FileBase {
                file_name: self.file_base.file_name.clone(),
                content: self.file_base.content.clone(),
                hash: self.file_base.hash,
                line_map: OnceLock::new(),
                line_info: OnceLock::new(),
            },
            version: self.version,
            kind: self.kind,
            matches_disk_text: self.matches_disk_text,
        }
    }
}

pub fn new_overlay(
    file_name: String,
    content: String,
    version: i32,
    kind: core::ScriptKind,
) -> Overlay {
    Overlay {
        file_base: FileBase {
            file_name,
            content: content.clone(),
            hash: xxh3::xxh3_128(content.as_bytes()),
            line_map: OnceLock::new(),
            line_info: OnceLock::new(),
        },
        version,
        kind,
        matches_disk_text: false,
    }
}

impl Overlay {
    pub fn text(&self) -> String {
        self.file_base.content()
    }

    // MatchesDiskText may return false negatives, but never false positives.
    pub fn compute_matches_disk_text(&self, fs: &dyn vfs::Fs) -> (bool, bool) {
        if tspath::is_dynamic_file_name(&self.file_base.file_name) {
            return (false, false);
        }
        let (disk_content, ok) = fs.read_file(&self.file_base.file_name);
        if !ok {
            return (false, false);
        }
        (
            xxh3::xxh3_128(disk_content.as_bytes()) == self.file_base.hash,
            true,
        )
    }
}

impl lsconv::Script for Overlay {
    fn file_name(&self) -> &str {
        &self.file_base.file_name
    }

    fn text(&self) -> &str {
        &self.file_base.content
    }
}

impl FileContent for Overlay {
    fn content(&self) -> String {
        self.file_base.content()
    }

    fn hash(&self) -> u128 {
        self.file_base.hash()
    }
}

impl FileHandle for Overlay {
    fn file_name(&self) -> String {
        self.file_base.file_name()
    }

    fn version(&self) -> i32 {
        self.version
    }

    fn matches_disk_text(&self) -> bool {
        self.matches_disk_text
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn lsp_line_map(&self) -> &lsconv::LspLineMap {
        self.file_base.lsp_line_map()
    }

    fn ecma_line_info(&self) -> &sourcemap::ECMALineInfo {
        self.file_base.ecma_line_info()
    }

    fn kind(&self) -> core::ScriptKind {
        self.kind
    }
}

pub struct OverlayFs {
    pub to_path: Box<dyn Fn(String) -> tspath::Path + Send + Sync>,
    pub fs: Arc<dyn vfs::Fs + Send + Sync>,
    pub position_encoding: lsproto::PositionEncodingKind,

    overlays: RwLock<HashMap<tspath::Path, Arc<Overlay>>>,
}

pub fn new_overlay_fs(
    fs: Arc<dyn vfs::Fs + Send + Sync>,
    overlays: HashMap<tspath::Path, Arc<Overlay>>,
    position_encoding: lsproto::PositionEncodingKind,
    to_path: impl Fn(String) -> tspath::Path + Send + Sync + 'static,
) -> OverlayFs {
    OverlayFs {
        fs,
        position_encoding,
        overlays: RwLock::new(overlays),
        to_path: Box::new(to_path),
    }
}

impl OverlayFs {
    pub fn overlays(&self) -> HashMap<tspath::Path, Arc<Overlay>> {
        self.overlays
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }

    pub fn get_file(&self, file_name: String) -> Option<Box<dyn FileHandle>> {
        let overlays = self
            .overlays
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .clone();

        let path = (self.to_path)(file_name.clone());
        if let Some(overlay) = overlays.get(&path) {
            return Some(Box::new(overlay.as_ref().clone()));
        }

        let (content, ok) = self.fs.read_file(&file_name);
        if !ok {
            return None;
        }
        Some(Box::new(new_disk_file(file_name, content)))
    }

    pub fn process_changes(
        &self,
        changes: Vec<FileChange>,
    ) -> (FileChangeSummary, HashMap<tspath::Path, Arc<Overlay>>) {
        let mut overlays = self.overlays.write().unwrap_or_else(|err| err.into_inner());

        let mut result = FileChangeSummary::default();
        let mut new_overlays = overlays.clone();

        // Reduced collection of changes that occurred on a single file
        struct FileEvents {
            open_change: Option<FileChange>,
            close_change: Option<FileChange>,
            watch_changed: bool,
            changes: Vec<FileChange>,
            saved: bool,
            created: bool,
            deleted: bool,
        }

        let mut file_event_map: HashMap<String, FileEvents> = HashMap::new();

        for change in changes {
            let uri = change.uri.clone();
            let events = file_event_map.entry(uri.clone()).or_insert(FileEvents {
                open_change: None,
                close_change: None,
                watch_changed: false,
                changes: Vec::new(),
                saved: false,
                created: false,
                deleted: false,
            });

            if events.open_change.is_some() {
                panic!("should see no changes after open");
            }

            if !result.includes_watch_change_outside_node_modules
                && change.kind.is_watch_kind()
                && !uri.contains("/node_modules/")
            {
                result.includes_watch_change_outside_node_modules = true;
            }

            match change.kind {
                FileChangeKind::Open => {
                    if events.close_change.is_some() {
                        events.close_change = None;
                    }
                    events.open_change = Some(change);
                    events.watch_changed = false;
                    events.changes.clear();
                    events.saved = false;
                    events.created = false;
                    events.deleted = false;
                }
                FileChangeKind::Close => {
                    events.close_change = Some(change);
                    events.changes.clear();
                    events.saved = false;
                    events.watch_changed = false;
                }
                FileChangeKind::Change => {
                    if events.close_change.is_some() {
                        panic!("should see no changes after close");
                    }
                    events.changes.push(change);
                    events.saved = false;
                    events.watch_changed = false;
                }
                FileChangeKind::Save => {
                    events.saved = true;
                }
                FileChangeKind::WatchCreate => {
                    if events.deleted {
                        // Delete followed by create becomes a change
                        events.deleted = false;
                        events.watch_changed = true;
                    } else {
                        events.created = true;
                    }
                }
                FileChangeKind::WatchChange => {
                    if !events.created {
                        events.watch_changed = true;
                        events.saved = false;
                    }
                }
                FileChangeKind::WatchDelete => {
                    events.watch_changed = false;
                    events.saved = false;
                    // Delete after create cancels out
                    if events.created {
                        events.created = false;
                    } else {
                        events.deleted = true;
                    }
                }
            }
        }

        // Process deduplicated events per file
        for (uri, events) in file_event_map {
            let path = uri.path(self.fs.use_case_sensitive_file_names());
            let mut overlay = new_overlays.get(&path).cloned();

            if let Some(open_change) = events.open_change {
                if !result.opened.is_empty() || !result.reopened.is_empty() {
                    panic!("can only process one file open event at a time");
                }
                if let Some(existing) = &overlay {
                    if existing.content() != open_change.content {
                        result.changed.insert(uri.clone());
                    } else {
                        result.reopened = uri.clone();
                    }
                } else {
                    result.opened = uri.clone();
                }
                let language_kind: LanguageKind = open_change.language_kind.clone();
                new_overlays.insert(
                    path.clone(),
                    Arc::new(new_overlay(
                        uri.file_name(),
                        open_change.content,
                        open_change.version,
                        lsconv::language_kind_to_script_kind(language_kind),
                    )),
                );
                continue;
            }

            if events.close_change.is_some() {
                if overlay.is_none() {
                    panic!("overlay not found for closed file: {uri}");
                }
                result.closed.insert(uri.clone());
                new_overlays.remove(&path);
                overlay = None;
            }

            if events.watch_changed {
                if overlay.is_none() {
                    result.changed.insert(uri.clone());
                } else if overlay.is_some() && !events.saved {
                    let current = overlay.as_ref().unwrap();
                    let (matches_disk_text, _) =
                        current.compute_matches_disk_text(self.fs.as_ref());
                    if matches_disk_text != current.matches_disk_text() {
                        let mut new_overlay_value = new_overlay(
                            current.file_name(),
                            current.content(),
                            current.version(),
                            current.kind(),
                        );
                        new_overlay_value.matches_disk_text = matches_disk_text;
                        let new_overlay_value = Arc::new(new_overlay_value);
                        new_overlays.insert(path.clone(), new_overlay_value.clone());
                        overlay = Some(new_overlay_value);
                    }
                }
            }

            if !events.changes.is_empty() {
                result.changed.insert(uri.clone());
                if overlay.is_none() {
                    panic!("overlay not found for changed file: {uri}");
                }
                for change in events.changes {
                    let has_changes = !change.changes.is_empty();
                    for text_change in change.changes {
                        if let Some(partial_change) = text_change.partial {
                            let current = overlay.as_ref().unwrap().clone();
                            let new_content = {
                                let line_map_overlay = current.clone();
                                let converters =
                                    lsconv::new_converters(self.position_encoding, move |_| {
                                        line_map_overlay.lsp_line_map().clone()
                                    });
                                converters
                                    .from_lsp_text_change(current.as_ref(), &partial_change)
                                    .apply_to(&current.file_base.content)
                            };
                            overlay = Some(Arc::new(new_overlay(
                                current.file_name(),
                                new_content,
                                change.version,
                                current.kind(),
                            )));
                        } else if let Some(whole_change) = text_change.whole_document {
                            let current = overlay.as_ref().unwrap().clone();
                            overlay = Some(Arc::new(new_overlay(
                                current.file_name(),
                                whole_change.text,
                                change.version,
                                current.kind(),
                            )));
                        }
                    }
                    if has_changes {
                        let current = overlay.as_ref().unwrap();
                        let mut new_overlay_value = new_overlay(
                            current.file_name(),
                            current.content(),
                            change.version,
                            current.kind(),
                        );
                        new_overlay_value.version = change.version;
                        new_overlay_value.file_base.hash =
                            xxh3::xxh3_128(new_overlay_value.file_base.content.as_bytes());
                        new_overlay_value.matches_disk_text = false;
                        let new_overlay_value = Arc::new(new_overlay_value);
                        new_overlays.insert(path.clone(), new_overlay_value.clone());
                        overlay = Some(new_overlay_value);
                    }
                }
            }

            if events.saved {
                if overlay.is_none() {
                    panic!("overlay not found for saved file: {uri}");
                }
                let current = overlay.as_ref().unwrap();
                let mut new_overlay_value = new_overlay(
                    current.file_name(),
                    current.content(),
                    current.version(),
                    current.kind(),
                );
                new_overlay_value.matches_disk_text = true;
                new_overlays.insert(path.clone(), Arc::new(new_overlay_value));
            }

            if events.created && overlay.is_none() {
                result.created.insert(uri.clone());
            }

            if events.deleted && overlay.is_none() {
                result.deleted.insert(uri.clone());
            }
        }

        *overlays = new_overlays.clone();
        (result, new_overlays)
    }
}
