use ts_ast as ast;
use ts_checker as checker;
use ts_collections as collections;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_module as module;
use ts_modulespecifiers as modulespecifiers;
use ts_tspath as tspath;

use crate::autoimport::{
    ExistingImport, Export, ExportId, Fix, Registry, add_package_json_dependencies,
};
use crate::lsutil;

#[derive(Default)]
pub struct View<'a> {
    pub registry: Option<Registry>,
    pub importing_file: Option<ast::SourceFile>,
    pub program: Option<&'a compiler::Program>,
    pub preferences: lsutil::UserPreferences,
    pub project_key: tspath::Path,
    pub allowed_endings: Vec<modulespecifiers::ModuleSpecifierEnding>,
    pub conditions: Option<collections::Set<String>>,
    pub existing_imports: Option<collections::MultiMap<String, ExistingImport>>,
    pub should_use_require_for_fixes: Option<bool>,
    pub should_use_uri_style_node_core_modules: core::Tristate,
}

impl<'a> Clone for View<'a> {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
            importing_file: self
                .importing_file
                .as_ref()
                .map(ast::SourceFile::share_readonly),
            program: self.program,
            preferences: self.preferences.clone(),
            project_key: self.project_key.clone(),
            allowed_endings: self.allowed_endings.clone(),
            conditions: self.conditions.clone(),
            existing_imports: self.existing_imports.clone(),
            should_use_require_for_fixes: self.should_use_require_for_fixes,
            should_use_uri_style_node_core_modules: self.should_use_uri_style_node_core_modules,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FixAndExport {
    pub fix: Fix,
    pub export: Option<Export>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum QueryKind {
    #[default]
    WordPrefix = 0,
    ExactMatch = 1,
    CaseInsensitiveMatch = 2,
}

pub const QUERY_KIND_WORD_PREFIX: QueryKind = QueryKind::WordPrefix;
pub const QUERY_KIND_EXACT_MATCH: QueryKind = QueryKind::ExactMatch;
pub const QUERY_KIND_CASE_INSENSITIVE_MATCH: QueryKind = QueryKind::CaseInsensitiveMatch;

pub fn new_view<'a>(
    registry: Registry,
    importing_file: &ast::SourceFile,
    project_key: tspath::Path,
    program: &'a compiler::Program,
    preferences: modulespecifiers::UserPreferences,
) -> View<'a> {
    let mut view_preferences = lsutil::UserPreferences::default();
    view_preferences.import_module_specifier_preference =
        preferences.import_module_specifier_preference.clone();
    view_preferences.import_module_specifier_ending =
        preferences.import_module_specifier_ending.clone();
    view_preferences.auto_import_specifier_exclude_regexes =
        preferences.auto_import_specifier_exclude_regexes.clone();
    let default_resolution_mode =
        modulespecifiers::ModuleSpecifierGenerationHost::default_resolution_mode_for_file(
            program,
            importing_file,
        );
    let allowed_endings = modulespecifiers::get_allowed_endings_in_preferred_order(
        &preferences,
        program,
        program.compiler_options(),
        importing_file,
        "",
        default_resolution_mode,
    );
    let conditions = module::get_conditions(program.compiler_options(), default_resolution_mode);

    View {
        registry: Some(registry),
        importing_file: Some(importing_file.share_readonly()),
        program: Some(program),
        project_key,
        preferences: view_preferences,
        allowed_endings,
        conditions: Some(collections::new_set_from_items(conditions)),
        should_use_uri_style_node_core_modules: lsutil::should_use_uri_style_node_core_modules(
            importing_file,
            program,
        ),
        existing_imports: None,
        should_use_require_for_fixes: None,
    }
}

impl View<'_> {
    pub fn get_allowed_endings(&mut self) -> &[modulespecifiers::ModuleSpecifierEnding] {
        if self.allowed_endings.is_empty() {
            let Some(program) = self.program else {
                return &self.allowed_endings;
            };
            let Some(importing_file) = self.importing_file.as_ref() else {
                return &self.allowed_endings;
            };
            let default_resolution_mode =
                modulespecifiers::ModuleSpecifierGenerationHost::default_resolution_mode_for_file(
                    program,
                    importing_file,
                );
            self.allowed_endings = modulespecifiers::get_allowed_endings_in_preferred_order(
                &self.preferences.module_specifier_preferences(),
                program,
                program.compiler_options(),
                importing_file,
                "",
                default_resolution_mode,
            );
        }
        &self.allowed_endings
    }

    pub fn search(&self, query: &str, kind: QueryKind) -> Vec<Export> {
        let search_fn = |bucket: &crate::autoimport::RegistryBucket| -> Vec<Export> {
            let Some(index) = bucket.index.as_ref() else {
                return Vec::new();
            };
            match kind {
                QueryKind::WordPrefix => index.search_word_prefix(query),
                QueryKind::ExactMatch => index.find(query, true),
                QueryKind::CaseInsensitiveMatch => index.find(query, false),
            }
        };

        self.search_with(search_fn)
    }

    pub fn search_by_export_id(&self, id: ExportId) -> Vec<Export> {
        self.search_with(|bucket| {
            bucket
                .index
                .as_ref()
                .map(|index| {
                    index
                        .entries
                        .iter()
                        .filter(|e| e.export_id == id)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    pub fn search_with(
        &self,
        search_fn: impl Fn(&crate::autoimport::RegistryBucket) -> Vec<Export>,
    ) -> Vec<Export> {
        let mut results = Vec::new();

        let Some(registry) = self.registry.as_ref() else {
            return results;
        };
        let Some(importing_file) = self.importing_file.as_ref() else {
            return results;
        };

        if let Some(bucket) = registry.projects.get(&self.project_key) {
            let exports = search_fn(bucket);
            results.reserve(exports.len());
            for e in exports {
                if e.module_id() == importing_file.path() {
                    // Don't auto-import from the importing file itself
                    continue;
                }
                results.push(e);
            }
        }

        // Compute the set of packages accessible to the importing file.
        // This includes packages from package.json dependencies (aggregated from ancestor directories)
        // plus packages that are directly imported by the project's program files.
        // If no package.json is found, allowedPackages remains nil and all packages are allowed.
        let mut allowed_packages: Option<collections::Set<String>> = None;
        let mut dir_path = tspath::get_directory_path(&importing_file.path());
        loop {
            if let Some(dir) = registry.directories.get(&dir_path) {
                if let Some(package_json) = dir.package_json.as_ref() {
                    if let Some(contents) = package_json
                        .exists()
                        .then(|| package_json.get_contents())
                        .flatten()
                        .filter(|contents| contents.parseable)
                    {
                        // Initialize to empty set if this is the first package.json we've seen
                        if allowed_packages.is_none() {
                            allowed_packages = Some(collections::Set::default());
                        }
                        add_package_json_dependencies(contents, allowed_packages.as_mut().unwrap());
                    }
                }
            }
            let parent = tspath::get_directory_path(&dir_path);
            if parent == dir_path {
                break;
            }
            dir_path = parent;
        }
        // If we found at least one package.json, also include packages directly imported by the project
        if let Some(allowed_packages) = allowed_packages.as_mut() {
            if let Some(bucket) = registry.projects.get(&self.project_key) {
                *allowed_packages =
                    allowed_packages.unioned_with(bucket.resolved_package_names.as_ref());
            }
        }

        let mut exclude_packages = collections::Set::default();
        let mut dir_path = tspath::get_directory_path(&importing_file.path());
        loop {
            if let Some(node_modules_bucket) = registry.node_modules.get(&dir_path) {
                let exports = search_fn(node_modules_bucket);
                results.reserve(exports.len());
                for e in exports {
                    // Exclude packages found in lower node_modules (shadowing)
                    if exclude_packages.has(&e.package_name) {
                        continue;
                    }
                    // If allowedPackages is nil, no package.json was found, so include all packages.
                    // Otherwise, only include packages that are dependencies or directly imported.
                    if let Some(allowed_packages) = allowed_packages.as_ref() {
                        if !allowed_packages.has(&e.package_name) {
                            continue;
                        }
                    }
                    results.push(e);
                }
                // As we go up the directory tree, exclude packages found in lower node_modules
                for pkg_name in node_modules_bucket.package_files.keys() {
                    exclude_packages.add(pkg_name.clone());
                }
            }
            let parent = tspath::get_directory_path(&dir_path);
            if parent == dir_path {
                break;
            }
            dir_path = parent;
        }
        results
    }

    pub fn get_completions(
        &self,
        checker: &mut checker::Checker<'_, '_>,
        prefix: &str,
        position: lsproto::Position,
        for_jsx: bool,
        is_type_only_location: bool,
    ) -> Vec<FixAndExport> {
        let results = self.search(prefix, QueryKind::WordPrefix);
        let results_len = results.len();

        let mut grouped: std::collections::HashMap<(ExportId, String, String), Vec<Export>> =
            std::collections::HashMap::with_capacity(results_len);

        'outer: for e in results {
            let name = e.name().to_string();
            if for_jsx && !((name.as_bytes()[0] as char).is_uppercase() || e.is_renameable()) {
                continue;
            }
            let mut target = e.export_id.clone();
            if e.target != ExportId::default() {
                target = e.target.clone();
            }
            let mut ambient_module_or_package_name = if !e.ambient_module_name().is_empty() {
                e.ambient_module_name().to_string()
            } else {
                e.package_name.clone()
            };
            if (e.package_name == "@types/node" || e.path.contains("/node_modules/@types/node/"))
                && core::UNPREFIXED_NODE_CORE_MODULES
                    .contains(&ambient_module_or_package_name.as_str())
            {
                ambient_module_or_package_name = format!("node:{ambient_module_or_package_name}");
            }
            let key = (target.clone(), name, ambient_module_or_package_name);
            if let Some(existing) = grouped.get_mut(&key) {
                for i in 0..existing.len() {
                    if e.export_id == existing[i].export_id {
                        existing[i] = Export {
                            export_id: e.export_id.clone(),
                            module_file_name: e.module_file_name.clone(),
                            package_name: e.package_name.clone(),
                            is_type_only: e.is_type_only || existing[i].is_type_only,
                            syntax: std::cmp::min(e.syntax, existing[i].syntax),
                            flags: e.flags | existing[i].flags,
                            script_element_kind: std::cmp::min(
                                e.script_element_kind,
                                existing[i].script_element_kind,
                            ),
                            script_element_kind_modifiers: e.script_element_kind_modifiers
                                | existing[i].script_element_kind_modifiers,
                            local_name: e.local_name.clone(),
                            target: e.target.clone(),
                            path: e.path.clone(),
                            through: String::new(),
                        };
                        continue 'outer;
                    }
                }
            }
            grouped.entry(key).or_default().push(e);
        }

        let mut fixes = Vec::with_capacity(results_len);
        let compare_fixes =
            |a: &FixAndExport, b: &FixAndExport| self.compare_fixes_for_ranking(&a.fix, &b.fix);

        for exps in grouped.into_values() {
            let mut fixes_for_group = Vec::new();
            for e in exps {
                for fix in
                    self.get_fixes(checker, &e, for_jsx, is_type_only_location, Some(&position))
                {
                    fixes_for_group.push(FixAndExport {
                        fix,
                        export: Some(e.clone()),
                    });
                }
            }
            fixes.extend(core::min_all_func(&fixes_for_group, compare_fixes));
        }

        fixes.sort_by(|a, b| self.compare_fixes_for_sorting(&a.fix, &b.fix).cmp(&0));
        fixes
    }
}
