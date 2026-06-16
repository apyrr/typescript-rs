use std::collections::{HashMap, HashSet};

use ts_ast as ast;
use ts_core as core;
use ts_module as module;
use ts_tspath as tspath;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnownDirectoryLink {
    // Matches the casing returned by realpath. Always has a trailing separator.
    pub real: String,
    // to_path(real), cached to avoid repeated recomputation. Always has a trailing separator.
    pub real_path: tspath::Path,
}

#[derive(Clone, Debug, Default)]
pub struct KnownSymlinks {
    pub directories: HashMap<tspath::Path, Option<KnownDirectoryLink>>,
    pub directories_by_realpath: HashMap<tspath::Path, HashSet<String>>,
    pub files: HashMap<tspath::Path, String>,
    pub files_by_realpath: HashMap<tspath::Path, HashSet<String>>,
    pub cwd: String,
    pub use_case_sensitive_file_names: bool,
}

impl KnownSymlinks {
    pub fn has_directory(&self, symlink_path: tspath::Path) -> bool {
        self.directories
            .contains_key(&tspath::ensure_trailing_directory_separator(&symlink_path))
    }

    pub fn directories(&self) -> &HashMap<tspath::Path, Option<KnownDirectoryLink>> {
        &self.directories
    }

    pub fn directories_mut(&mut self) -> &mut HashMap<tspath::Path, Option<KnownDirectoryLink>> {
        &mut self.directories
    }

    pub fn directories_by_realpath(&self) -> &HashMap<tspath::Path, HashSet<String>> {
        &self.directories_by_realpath
    }

    pub fn files(&self) -> &HashMap<tspath::Path, String> {
        &self.files
    }

    pub fn files_by_realpath(&self) -> &HashMap<tspath::Path, HashSet<String>> {
        &self.files_by_realpath
    }

    pub fn set_directory(
        &mut self,
        symlink: String,
        symlink_path: tspath::Path,
        real_directory: Option<KnownDirectoryLink>,
    ) {
        if let Some(real_directory) = &real_directory
            && !self.directories.contains_key(&symlink_path)
        {
            self.directories_by_realpath
                .entry(real_directory.real_path.clone())
                .or_default()
                .insert(symlink);
        }
        self.directories.insert(symlink_path, real_directory);
    }

    pub fn set_file(&mut self, symlink: String, symlink_path: tspath::Path, realpath: String) {
        if !self.files.contains_key(&symlink_path) {
            let realpath_path =
                tspath::to_path(&realpath, &self.cwd, self.use_case_sensitive_file_names);
            self.files_by_realpath
                .entry(realpath_path)
                .or_default()
                .insert(symlink);
        }
        self.files.insert(symlink_path, realpath);
    }

    pub fn set_symlinks_from_resolutions<FM, FT>(
        &mut self,
        for_each_resolved_module: FM,
        for_each_resolved_type_reference_directive: FT,
    ) where
        FM: FnOnce(
            &mut dyn FnMut(&module::ResolvedModule, &str, core::ResolutionMode, tspath::Path),
            Option<&ast::SourceFile>,
        ),
        FT: FnOnce(
            &mut dyn FnMut(
                &module::ResolvedTypeReferenceDirective,
                &str,
                core::ResolutionMode,
                tspath::Path,
            ),
            Option<&ast::SourceFile>,
        ),
    {
        for_each_resolved_module(
            &mut |resolution, _module_name, _mode, _file_path| {
                self.process_resolution(
                    resolution.original_path.clone(),
                    resolution.resolved_file_name.clone(),
                );
            },
            None,
        );
        for_each_resolved_type_reference_directive(
            &mut |resolution, _module_name, _mode, _file_path| {
                self.process_resolution(
                    resolution.original_path.clone(),
                    resolution.resolved_file_name.clone(),
                );
            },
            None,
        );
    }

    pub fn process_resolution(&mut self, original_path: String, resolved_file_name: String) {
        if original_path.is_empty() || resolved_file_name.is_empty() {
            return;
        }

        self.set_file(
            original_path.clone(),
            tspath::to_path(
                &original_path,
                &self.cwd,
                self.use_case_sensitive_file_names,
            ),
            resolved_file_name.clone(),
        );

        let (common_resolved, common_original) =
            self.guess_directory_symlink(&resolved_file_name, &original_path, &self.cwd);
        if !common_resolved.is_empty() && !common_original.is_empty() {
            let symlink_path = tspath::to_path(
                &common_original,
                &self.cwd,
                self.use_case_sensitive_file_names,
            );
            if !tspath::contains_ignored_path(&symlink_path) {
                self.set_directory(
                    common_original.clone(),
                    tspath::ensure_trailing_directory_separator(&symlink_path),
                    Some(KnownDirectoryLink {
                        real: tspath::ensure_trailing_directory_separator(&common_resolved),
                        real_path: tspath::ensure_trailing_directory_separator(&tspath::to_path(
                            &common_resolved,
                            &self.cwd,
                            self.use_case_sensitive_file_names,
                        )),
                    }),
                );
            }
        }
    }

    pub fn guess_directory_symlink(&self, a: &str, b: &str, cwd: &str) -> (String, String) {
        let mut a_parts = get_path_components(&tspath::get_normalized_absolute_path(a, cwd));
        let mut b_parts = get_path_components(&tspath::get_normalized_absolute_path(b, cwd));
        let mut is_directory = false;

        while a_parts.len() >= 2
            && b_parts.len() >= 2
            && !self.is_node_modules_or_scoped_package_directory(&a_parts[a_parts.len() - 2])
            && !self.is_node_modules_or_scoped_package_directory(&b_parts[b_parts.len() - 2])
            && tspath::get_canonical_file_name(
                &a_parts[a_parts.len() - 1],
                self.use_case_sensitive_file_names,
            ) == tspath::get_canonical_file_name(
                &b_parts[b_parts.len() - 1],
                self.use_case_sensitive_file_names,
            )
        {
            a_parts.pop();
            b_parts.pop();
            is_directory = true;
        }

        if is_directory {
            (
                get_path_from_components(&a_parts),
                get_path_from_components(&b_parts),
            )
        } else {
            (String::new(), String::new())
        }
    }

    pub fn is_node_modules_or_scoped_package_directory(&self, s: &str) -> bool {
        !s.is_empty()
            && (tspath::get_canonical_file_name(s, self.use_case_sensitive_file_names)
                == "node_modules"
                || s.starts_with('@'))
    }
}

pub fn new_known_symlink(
    current_directory: &str,
    use_case_sensitive_file_names: bool,
) -> KnownSymlinks {
    KnownSymlinks {
        cwd: current_directory.to_string(),
        use_case_sensitive_file_names,
        ..KnownSymlinks::default()
    }
}

fn get_path_components(path: &str) -> Vec<String> {
    if path == "/" {
        return vec!["/".to_string()];
    }
    let mut parts = Vec::new();
    let mut rest = path;
    if let Some(stripped) = rest.strip_prefix('/') {
        parts.push("/".to_string());
        rest = stripped;
    }
    parts.extend(
        rest.split('/')
            .filter(|part| !part.is_empty())
            .map(str::to_string),
    );
    parts
}

fn get_path_from_components(parts: &[String]) -> String {
    if parts.is_empty() {
        return String::new();
    }
    if parts[0] == "/" {
        if parts.len() == 1 {
            return "/".to_string();
        }
        return format!("/{}", parts[1..].join("/"));
    }
    parts.join("/")
}
