use std::collections::HashMap;

use crate::{
    CheckerShape, MatchingMode, ModuleSpecifierEnding, ModuleSpecifierGenerationHost,
    ModuleSpecifierOptions, ModuleSpecifierPreferences, ModuleSymbolData, RelativePreferenceKind,
    ResultKind, SourceFileForSpecifierGeneration, UserPreferences,
    get_js_extension_for_declaration_file_extension, get_relative_path_if_in_same_volume,
    is_path_relative_to_parent, package_json_paths_are_equal, prefers_ts_extension,
    replace_first_star, try_get_real_file_name_for_non_js_declaration_file_name,
};
use ts_ast as ast;
use ts_core as core;
use ts_packagejson as packagejson;
use ts_tspath as tspath;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Info {
    pub use_case_sensitive_file_names: bool,
    pub importing_source_file_name: String,
    pub source_directory: String,
}

#[derive(Clone, Debug, Default)]
pub struct GetModuleSpecifiersOptions {
    pub user_preferences: UserPreferences,
    pub options: ModuleSpecifierOptions,
    pub for_auto_imports: bool,
}

pub fn get_module_specifiers(
    module_symbol: &ModuleSymbolData,
    checker: &mut impl CheckerShape,
    compiler_options: &core::CompilerOptions,
    importing_source_file: &impl SourceFileForSpecifierGeneration,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    specifier_options: GetModuleSpecifiersOptions,
) -> Vec<String> {
    get_module_specifiers_with_info(
        module_symbol,
        checker,
        compiler_options,
        importing_source_file,
        host,
        specifier_options,
    )
    .0
}

pub fn get_module_specifiers_with_info(
    module_symbol: &ModuleSymbolData,
    checker: &mut impl CheckerShape,
    compiler_options: &core::CompilerOptions,
    importing_source_file: &impl SourceFileForSpecifierGeneration,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    specifier_options: GetModuleSpecifiersOptions,
) -> (Vec<String>, ResultKind) {
    let GetModuleSpecifiersOptions {
        user_preferences,
        options,
        for_auto_imports,
    } = specifier_options;

    let ambient = try_get_module_name_from_ambient_module(module_symbol, checker);
    if !ambient.is_empty() {
        if for_auto_imports
            && crate::is_excluded_by_regex(
                &ambient,
                &user_preferences.auto_import_specifier_exclude_regexes,
            )
        {
            return (Vec::new(), ResultKind::Ambient);
        }
        return (vec![ambient], ResultKind::Ambient);
    }

    let Some(module_store) = module_symbol
        .value_declaration
        .or_else(|| module_symbol.declarations.first().copied())
        .and_then(|declaration| checker.source_file_store(declaration))
    else {
        return (Vec::new(), ResultKind::None);
    };
    let Some(module_source_file) =
        get_source_file_node_of_module_symbol_data(module_store, module_symbol)
    else {
        return (Vec::new(), ResultKind::None);
    };

    // Use original source file name when file is from project reference output.
    let module_source_file_data = module_store.as_source_file(module_source_file);
    let module_file_name = host.source_of_project_reference_if_output_included(&FileNameOnly {
        file_name: module_source_file_data.file_name(),
        path: module_source_file_data.path(),
    });

    get_module_specifiers_for_file_with_info(
        importing_source_file,
        &module_file_name,
        compiler_options,
        host,
        user_preferences,
        options,
        for_auto_imports,
    )
}

pub fn get_module_specifiers_for_file_with_info(
    importing_source_file: &impl SourceFileForSpecifierGeneration,
    module_file_name: &str,
    compiler_options: &core::CompilerOptions,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    user_preferences: UserPreferences,
    options: ModuleSpecifierOptions,
    for_auto_imports: bool,
) -> (Vec<String>, ResultKind) {
    let importing_source_file_name = importing_source_file.file_name();
    let importing_name = host.source_of_project_reference_if_output_included(&FileNameOnly {
        file_name: importing_source_file_name,
        path: importing_source_file.path(),
    });
    let module_paths = get_all_module_paths_worker(
        get_info(&importing_name, host),
        module_file_name,
        host,
        compiler_options,
        options,
    );

    compute_module_specifiers(
        &module_paths,
        compiler_options,
        importing_source_file,
        host,
        user_preferences,
        options,
        for_auto_imports,
    )
}

pub fn try_get_module_name_from_ambient_module(
    module_symbol: &ModuleSymbolData,
    checker: &mut impl CheckerShape,
) -> String {
    for declaration in &module_symbol.declarations {
        let Some(store) = checker.source_file_store(*declaration) else {
            continue;
        };
        let name_text = store
            .name(*declaration)
            .map(|name| store.text(name))
            .unwrap_or_default();
        if ast::is_module_with_string_literal_name(store, *declaration)
            && (!ast::is_module_augmentation_external(store, *declaration)
                || !tspath::is_external_module_name_relative(&name_text))
        {
            return name_text;
        }
    }

    // the module could be a namespace, which is export through "export=" from an ambient module.
    /*
     * declare module "m" {
     *     namespace ns {
     *         class c {}
     *     }
     *     export = ns;
     * }
     */
    // `import {c} from "m";` is valid, in which case, `moduleSymbol` is "ns", but the module name should be "m"
    for declaration in &module_symbol.declarations {
        let Some(store) = checker.source_file_store(*declaration) else {
            continue;
        };
        if !ast::is_module_declaration(store, *declaration) {
            continue;
        }

        let Some((expression, declaration_symbol, possible_container_name)) = ({
            let Some(possible_container) =
                ast::find_ancestor(store, Some(*declaration), |store, node| {
                    ast::is_module_with_string_literal_name(store, node)
                })
            else {
                continue;
            };
            let Some(parent) = store.parent(possible_container) else {
                continue;
            };
            if !ast::is_source_file(store, parent) {
                continue;
            }
            let possible_container_name = store
                .name(possible_container)
                .map(|name| store.text(name))
                .unwrap_or_default();

            let Some(container_symbol) = checker.source_node_symbol(possible_container) else {
                continue;
            };
            let Some(export_equals_symbol) = checker.lookup_source_symbol_export(
                container_symbol,
                ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS,
            ) else {
                continue;
            };
            let Some(export_assignment_decl) =
                checker.symbol_value_declaration(export_equals_symbol)
            else {
                continue;
            };
            let Some(export_store) = checker.source_file_store(export_assignment_decl) else {
                continue;
            };
            if !ast::is_export_assignment(export_store, export_assignment_decl) {
                continue;
            }
            let Some(expression) = export_store.expression(export_assignment_decl) else {
                continue;
            };
            let declaration_symbol = checker.source_node_symbol(*declaration);
            Some((expression, declaration_symbol, possible_container_name))
        }) else {
            continue;
        };
        let Some(mut export_symbol) = checker.get_symbol_at_location(expression) else {
            continue;
        };
        // TODO upstream: possible strada bug - isn't this insufficient in the presence of merge symbols?
        if export_symbol.is_alias()
            && let Some(aliased) = checker.get_aliased_symbol_at_location(expression)
        {
            export_symbol = aliased;
        }
        if declaration_symbol
            .is_some_and(|declaration_symbol| export_symbol.identity() == declaration_symbol)
        {
            return possible_container_name;
        }
    }
    String::new()
}

fn get_source_file_node_of_module_symbol_data(
    store: &ast::AstStore,
    module_symbol: &ModuleSymbolData,
) -> Option<ast::Node> {
    module_symbol
        .value_declaration
        .or_else(|| module_symbol.declarations.first().copied())
        .and_then(|declaration| ast::get_source_file_of_node(store, Some(declaration)))
}

pub(crate) fn get_info(
    importing_source_file_name: &str,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
) -> Info {
    let source_directory = tspath::get_directory_path(importing_source_file_name);
    Info {
        importing_source_file_name: importing_source_file_name.to_string(),
        source_directory,
        use_case_sensitive_file_names: host.use_case_sensitive_file_names(),
    }
}

pub fn get_all_module_paths(
    info: Info,
    imported_file_name: &str,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    compiler_options: &core::CompilerOptions,
    _preferences: UserPreferences,
    options: ModuleSpecifierOptions,
) -> Vec<crate::ModulePath> {
    get_all_module_paths_worker(info, imported_file_name, host, compiler_options, options)
}

pub fn get_all_module_paths_worker(
    info: Info,
    imported_file_name: &str,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    _compiler_options: &core::CompilerOptions,
    _options: ModuleSpecifierOptions,
) -> Vec<crate::ModulePath> {
    let paths = get_each_file_name_of_module(
        &info.importing_source_file_name,
        imported_file_name,
        host,
        true,
    );
    let mut all_file_names: HashMap<String, crate::ModulePath> = paths
        .iter()
        .map(|path| (path.file_name.clone(), path.clone()))
        .collect();

    let mut sorted_paths = Vec::with_capacity(paths.len());
    let mut directory = info.source_directory.clone();
    while !all_file_names.is_empty() {
        let directory_start = tspath::ensure_trailing_directory_separator(&directory);
        let mut paths_in_directory = Vec::new();
        let keys = all_file_names.keys().cloned().collect::<Vec<_>>();
        for file_name in keys {
            if file_name.starts_with(&directory_start)
                && let Some(path) = all_file_names.remove(&file_name)
            {
                paths_in_directory.push(path);
            }
        }
        if !paths_in_directory.is_empty() {
            paths_in_directory.sort_by(|a, b| {
                crate::compare_paths_by_redirect(a, b, info.use_case_sensitive_file_names)
            });
            sorted_paths.extend(paths_in_directory);
        }
        let new_directory = tspath::get_directory_path(&directory);
        if new_directory == directory {
            break;
        }
        directory = new_directory;
    }
    if !all_file_names.is_empty() {
        let mut remaining_paths = all_file_names.into_values().collect::<Vec<_>>();
        remaining_paths.sort_by(|a, b| {
            crate::compare_paths_by_redirect(a, b, info.use_case_sensitive_file_names)
        });
        sorted_paths.extend(remaining_paths);
    }
    sorted_paths
}

pub fn contains_ignored_path(s: &str) -> bool {
    // containsIgnoredPath checks if a path contains patterns that should be ignored.
    // This is a local helper that duplicates tspath.ContainsIgnoredPath for performance.
    s.contains("/node_modules/.") || s.contains("/.git") || s.contains(".#")
}

pub fn contains_node_modules(s: &str) -> bool {
    // ContainsNodeModules checks if a path contains the node_modules directory.
    s.contains("/node_modules/")
}

pub fn get_each_file_name_of_module(
    importing_file_name: &str,
    imported_file_name: &str,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    prefer_symlinks: bool,
) -> Vec<crate::ModulePath> {
    let cwd = host.current_directory();
    let imported_path = tspath::to_path(
        imported_file_name,
        &cwd,
        host.use_case_sensitive_file_names(),
    );
    let mut reference_redirect = String::new();
    if let Some(output_and_reference) = host.project_reference_from_source(imported_path.clone())
        && !output_and_reference.output_dts.is_empty()
    {
        reference_redirect = output_and_reference.output_dts;
    }

    let redirects = host.redirect_targets(imported_path);
    let mut imported_file_names = Vec::with_capacity(2 + redirects.len());
    if !reference_redirect.is_empty() {
        imported_file_names.push(reference_redirect.clone());
    }
    imported_file_names.push(imported_file_name.to_string());
    imported_file_names.extend(redirects);
    let targets = imported_file_names
        .iter()
        .map(|file_name| tspath::get_normalized_absolute_path(file_name, &cwd))
        .collect::<Vec<_>>();
    let mut should_filter_ignored_paths =
        !targets.iter().all(|target| contains_ignored_path(target));

    let mut results = Vec::with_capacity(2);
    if !prefer_symlinks {
        for p in &targets {
            if !(should_filter_ignored_paths && contains_ignored_path(p)) {
                results.push(crate::ModulePath {
                    file_name: p.clone(),
                    is_in_node_modules: contains_node_modules(p),
                    is_redirect: p == &reference_redirect,
                });
            }
        }
    }

    let symlink_cache = host.symlink_cache();
    let full_imported_file_name = tspath::get_normalized_absolute_path(imported_file_name, &cwd);
    if let Some(symlink_cache) = symlink_cache {
        let _ = tspath::for_each_ancestor_directory_stopping_at_global_cache(
            &host.global_typings_cache_location(),
            tspath::get_directory_path(&full_imported_file_name),
            |real_path_directory| {
                let real_path_directory_path =
                    tspath::ensure_trailing_directory_separator(&tspath::to_path(
                        real_path_directory,
                        &cwd,
                        host.use_case_sensitive_file_names(),
                    ));
                let Some(symlink_set) = symlink_cache
                    .directories_by_realpath()
                    .get(&real_path_directory_path)
                else {
                    return ((), false);
                };

                // Don't want to a package to globally import from itself (importNameCodeFix_symlink_own_package.ts)
                if tspath::starts_with_directory(
                    importing_file_name,
                    real_path_directory,
                    host.use_case_sensitive_file_names(),
                ) {
                    return ((), true);
                }

                for target in &targets {
                    if !tspath::starts_with_directory(
                        target,
                        real_path_directory,
                        host.use_case_sensitive_file_names(),
                    ) {
                        continue;
                    }

                    let relative = tspath::get_relative_path_from_directory(
                        real_path_directory,
                        target,
                        &tspath::ComparePathsOptions {
                            use_case_sensitive_file_names: host.use_case_sensitive_file_names(),
                            current_directory: cwd.clone(),
                        },
                    );
                    for symlink_directory in symlink_set {
                        let option = tspath::resolve_path(symlink_directory, &[&relative]);
                        results.push(crate::ModulePath {
                            file_name: option.clone(),
                            is_in_node_modules: contains_node_modules(&option),
                            is_redirect: target == &reference_redirect,
                        });
                        should_filter_ignored_paths = true;
                    }
                }

                ((), false)
            },
        );
    }

    if prefer_symlinks {
        for p in &targets {
            if !(should_filter_ignored_paths && contains_ignored_path(p)) {
                results.push(crate::ModulePath {
                    file_name: p.clone(),
                    is_in_node_modules: contains_node_modules(p),
                    is_redirect: p == &reference_redirect,
                });
            }
        }
    }

    results
}

pub fn compute_module_specifiers(
    module_paths: &[crate::ModulePath],
    compiler_options: &core::CompilerOptions,
    importing_source_file: &impl SourceFileForSpecifierGeneration,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    user_preferences: UserPreferences,
    options: ModuleSpecifierOptions,
    for_auto_import: bool,
) -> (Vec<String>, ResultKind) {
    let info = get_info(&importing_source_file.file_name(), host);
    let preferences = crate::get_module_specifier_preferences(
        &user_preferences,
        host,
        compiler_options,
        importing_source_file,
        "",
    );

    let mut existing_specifier = String::new();
    'module_paths: for module_path in module_paths {
        let target_path = tspath::to_path(
            &module_path.file_name,
            &host.current_directory(),
            info.use_case_sensitive_file_names,
        );
        let importing_file = FileNameOnly {
            file_name: importing_source_file.file_name(),
            path: importing_source_file.path(),
        };
        let mut existing_import = None;
        for import_specifier in importing_source_file.imports() {
            let Some(resolved_module) =
                host.resolved_module_from_module_specifier(&importing_file, &import_specifier)
            else {
                continue;
            };
            if resolved_module.is_resolved()
                && tspath::to_path(
                    &resolved_module.resolved_file_name,
                    &host.current_directory(),
                    info.use_case_sensitive_file_names,
                ) == target_path
            {
                existing_import = Some(import_specifier);
                break;
            }
        }
        if let Some(existing_import) = existing_import {
            let existing_import_text = importing_source_file.import_text(&existing_import);
            if preferences.relative_preference == RelativePreferenceKind::NonRelative
                && tspath::path_is_relative(&existing_import_text)
            {
                // If the preference is for non-relative and the module specifier is relative, ignore it
                continue;
            }
            let existing_mode = host.mode_for_usage_location(&importing_file, &existing_import);
            let target_mode = if options.override_import_mode == core::RESOLUTION_MODE_NONE {
                host.default_resolution_mode_for_file(&importing_file)
            } else {
                options.override_import_mode
            };
            if existing_mode != target_mode
                && existing_mode != core::RESOLUTION_MODE_NONE
                && target_mode != core::RESOLUTION_MODE_NONE
            {
                // If the candidate import mode doesn't match the mode we're generating for, don't consider it
                continue;
            }
            existing_specifier = existing_import_text;
            break 'module_paths;
        }
    }

    if !existing_specifier.is_empty() {
        return (vec![existing_specifier], ResultKind::None);
    }

    let imported_file_is_in_node_modules = module_paths.iter().any(|p| p.is_in_node_modules);

    // Module specifier priority:
    //   1. "Bare package specifiers" (e.g. "@foo/bar") resulting from a path through node_modules to a package.json's "types" entry
    //   2. Specifiers generated using "paths" from tsconfig
    //   3. Non-relative specfiers resulting from a path through node_modules (e.g. "@foo/bar/path/to/file")
    //   4. Relative paths
    let mut paths_specifiers = Vec::new();
    let mut redirect_paths_specifiers = Vec::new();
    let mut node_modules_specifiers = Vec::new();
    let mut relative_specifiers = Vec::new();

    for module_path in module_paths {
        let mut specifier = String::new();
        if module_path.is_in_node_modules {
            specifier = try_get_module_name_as_node_module(
                module_path,
                &info,
                importing_source_file,
                host,
                compiler_options,
                NodeModuleSpecifierOptions {
                    user_preferences: user_preferences.clone(),
                    package_name_only: false,
                    override_mode: options.override_import_mode,
                },
            );
        }
        if !(specifier.is_empty()
            || for_auto_import
                && crate::is_excluded_by_regex(&specifier, &preferences.exclude_regexes))
        {
            node_modules_specifiers.push(specifier.clone());
            if module_path.is_redirect {
                return (node_modules_specifiers, ResultKind::NodeModules);
            }
        }

        let import_mode = if options.override_import_mode == core::RESOLUTION_MODE_NONE {
            host.default_resolution_mode_for_file(&FileNameOnly {
                file_name: importing_source_file.file_name(),
                path: importing_source_file.path(),
            })
        } else {
            options.override_import_mode
        };
        let local = get_local_module_specifier_worker(
            &module_path.file_name,
            &info,
            compiler_options,
            host,
            import_mode,
            &preferences,
            module_path.is_redirect || !specifier.is_empty(),
        );
        if local.is_empty()
            || for_auto_import && crate::is_excluded_by_regex(&local, &preferences.exclude_regexes)
        {
            continue;
        }
        if module_path.is_redirect {
            redirect_paths_specifiers.push(local);
        } else if crate::path_is_bare_specifier(&local) {
            if contains_node_modules(&local) {
                relative_specifiers.push(local);
            } else {
                paths_specifiers.push(local);
            }
        } else if for_auto_import
            || !imported_file_is_in_node_modules
            || module_path.is_in_node_modules
        {
            relative_specifiers.push(local);
        }
    }

    if !paths_specifiers.is_empty() {
        return (paths_specifiers, ResultKind::Paths);
    }
    if !redirect_paths_specifiers.is_empty() {
        return (redirect_paths_specifiers, ResultKind::Redirect);
    }
    if !node_modules_specifiers.is_empty() {
        return (node_modules_specifiers, ResultKind::NodeModules);
    }
    (relative_specifiers, ResultKind::Relative)
}

pub fn get_local_module_specifier(
    module_file_name: &str,
    importing_file_name: &str,
    compiler_options: &core::CompilerOptions,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    user_preferences: UserPreferences,
) -> String {
    let info = get_info(importing_file_name, host);
    let source_file = SimpleSourceFileForSpecifierGeneration {
        file_name: importing_file_name.to_string(),
    };
    let import_mode = host.default_resolution_mode_for_file(&FileNameOnly {
        file_name: importing_file_name.to_string(),
        path: importing_file_name.to_string(),
    });
    let preferences = crate::get_module_specifier_preferences(
        &user_preferences,
        host,
        compiler_options,
        &source_file,
        "",
    );
    get_local_module_specifier_worker(
        module_file_name,
        &info,
        compiler_options,
        host,
        import_mode,
        &preferences,
        false,
    )
}

fn get_local_module_specifier_worker(
    module_file_name: &str,
    info: &Info,
    compiler_options: &core::CompilerOptions,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    import_mode: core::ResolutionMode,
    preferences: &ModuleSpecifierPreferences,
    paths_only: bool,
) -> String {
    let paths = &compiler_options.paths;
    let root_dirs = &compiler_options.root_dirs;

    if paths_only && paths.size() == 0 {
        return String::new();
    }

    let source_directory = &info.source_directory;
    let allowed_endings = &preferences.allowed_endings_in_preferred_order;
    let mut relative_path = String::new();
    if !root_dirs.is_empty() {
        relative_path = try_get_module_name_from_root_dirs(
            root_dirs,
            module_file_name,
            source_directory,
            allowed_endings,
            compiler_options,
            host,
        );
    }
    if relative_path.is_empty() {
        relative_path = process_ending(
            &crate::ensure_path_is_non_module_name(&tspath::get_relative_path_from_directory(
                source_directory,
                module_file_name,
                &tspath::ComparePathsOptions {
                    use_case_sensitive_file_names: host.use_case_sensitive_file_names(),
                    current_directory: host.current_directory(),
                },
            )),
            allowed_endings,
            compiler_options,
            Some(host),
        );
    }

    if (paths.size() == 0 && !compiler_options.get_resolve_package_json_imports())
        || preferences.relative_preference == RelativePreferenceKind::Relative
    {
        if paths_only {
            return String::new();
        }
        return relative_path;
    }

    let root = compiler_options.get_paths_base_path(&host.current_directory());
    let base_directory = tspath::get_normalized_absolute_path(&root, &host.current_directory());
    let relative_to_base_url = get_relative_path_if_in_same_volume(
        module_file_name,
        &base_directory,
        host.use_case_sensitive_file_names(),
    );
    if relative_to_base_url.is_empty() {
        if paths_only {
            return String::new();
        }
        return relative_path;
    }

    let mut from_package_json_imports = String::new();
    if !paths_only {
        from_package_json_imports = try_get_module_name_from_package_json_imports(
            module_file_name,
            source_directory,
            compiler_options,
            host,
            import_mode,
            prefers_ts_extension(allowed_endings),
        );
    }

    let mut from_paths = String::new();
    if (paths_only || from_package_json_imports.is_empty()) && paths.size() != 0 {
        from_paths = try_get_module_name_from_paths(
            &relative_to_base_url,
            paths,
            allowed_endings,
            &base_directory,
            host,
            compiler_options,
        );
    }

    if paths_only {
        return from_paths;
    }

    let maybe_non_relative = if !from_package_json_imports.is_empty() {
        from_package_json_imports.clone()
    } else {
        from_paths
    };
    if maybe_non_relative.is_empty() {
        return relative_path;
    }

    let relative_is_excluded =
        crate::is_excluded_by_regex(&relative_path, &preferences.exclude_regexes);
    let non_relative_is_excluded =
        crate::is_excluded_by_regex(&maybe_non_relative, &preferences.exclude_regexes);
    if !relative_is_excluded && non_relative_is_excluded {
        return relative_path;
    }
    if relative_is_excluded && !non_relative_is_excluded {
        return maybe_non_relative;
    }

    if preferences.relative_preference == RelativePreferenceKind::NonRelative
        && !tspath::path_is_relative(&maybe_non_relative)
    {
        return maybe_non_relative;
    }

    if preferences.relative_preference == RelativePreferenceKind::ExternalNonRelative
        && !tspath::path_is_relative(&maybe_non_relative)
    {
        let project_directory = if !compiler_options.config_file_path.is_empty() {
            tspath::to_path(
                &compiler_options.config_file_path,
                &host.current_directory(),
                host.use_case_sensitive_file_names(),
            )
        } else {
            tspath::to_path(
                &host.current_directory(),
                &host.current_directory(),
                host.use_case_sensitive_file_names(),
            )
        };
        let canonical_source_directory = tspath::to_path(
            source_directory,
            &host.current_directory(),
            host.use_case_sensitive_file_names(),
        );
        let module_path = tspath::to_path(
            module_file_name,
            &project_directory,
            host.use_case_sensitive_file_names(),
        );

        let source_is_internal = canonical_source_directory.starts_with(&project_directory);
        let target_is_internal = module_path.starts_with(&project_directory);
        if source_is_internal && !target_is_internal || !source_is_internal && target_is_internal {
            return maybe_non_relative;
        }

        let nearest_target_package_json = host.nearest_ancestor_directory_with_package_json(
            &tspath::get_directory_path(&module_path),
        );
        let nearest_source_package_json =
            host.nearest_ancestor_directory_with_package_json(source_directory);
        if !package_json_paths_are_equal(
            &nearest_target_package_json,
            &nearest_source_package_json,
            &tspath::ComparePathsOptions {
                use_case_sensitive_file_names: host.use_case_sensitive_file_names(),
                current_directory: host.current_directory(),
            },
        ) {
            return maybe_non_relative;
        }
        if !from_package_json_imports.is_empty() {
            return relative_path;
        }
    }

    if is_path_relative_to_parent(&maybe_non_relative)
        || crate::count_path_components(&relative_path)
            < crate::count_path_components(&maybe_non_relative)
    {
        return relative_path;
    }
    maybe_non_relative
}

pub fn process_ending(
    file_name: &str,
    allowed_endings: &[ModuleSpecifierEnding],
    options: &core::CompilerOptions,
    host: Option<&(impl ModuleSpecifierGenerationHost + ?Sized)>,
) -> String {
    if tspath::file_extension_is_one_of(
        file_name,
        &[
            tspath::EXTENSION_JSON,
            tspath::EXTENSION_MJS,
            tspath::EXTENSION_CJS,
        ],
    ) {
        return file_name.to_string();
    }

    let no_extension = tspath::remove_file_extension(file_name);
    if file_name == no_extension {
        return file_name.to_string();
    }

    let js_priority = allowed_endings
        .iter()
        .position(|ending| *ending == ModuleSpecifierEnding::JsExtension);
    let ts_priority = allowed_endings
        .iter()
        .position(|ending| *ending == ModuleSpecifierEnding::TsExtension);
    if tspath::file_extension_is_one_of(file_name, &[tspath::EXTENSION_MTS, tspath::EXTENSION_CTS])
        && ts_priority.is_some_and(|ts| js_priority.is_none_or(|js| ts < js))
    {
        return file_name.to_string();
    }
    if tspath::file_extension_is_one_of(
        file_name,
        &[tspath::EXTENSION_DMTS, tspath::EXTENSION_DCTS],
    ) {
        let input_ext = tspath::get_declaration_file_extension(file_name);
        let ext = get_js_extension_for_declaration_file_extension(&input_ext);
        return format!("{}{}", tspath::remove_extension(file_name, &input_ext), ext);
    }
    if tspath::file_extension_is_one_of(file_name, &[tspath::EXTENSION_MTS, tspath::EXTENSION_CTS])
    {
        return format!(
            "{no_extension}{}",
            get_js_extension_for_file(file_name, options)
        );
    }
    if !tspath::file_extension_is_one_of(file_name, &[tspath::EXTENSION_DTS])
        && tspath::file_extension_is_one_of(file_name, &[tspath::EXTENSION_TS])
        && file_name.contains(".d.")
    {
        // `foo.d.json.ts` and the like - remap back to `foo.json`
        let result = try_get_real_file_name_for_non_js_declaration_file_name(file_name);
        if !result.is_empty() {
            return result;
        }
    }

    match allowed_endings.first().copied().unwrap_or_default() {
        ModuleSpecifierEnding::Minimal => {
            let without_index = no_extension.strip_suffix("/index").unwrap_or(&no_extension);
            if let Some(host) = host
                && without_index != no_extension
                && try_get_any_file_from_path_for_host(host, without_index)
            {
                // Can't remove index if there's a file by the same name as the directory.
                // Probably more callers should pass `host` so we can determine this?
                return no_extension;
            }
            without_index.to_string()
        }
        ModuleSpecifierEnding::Index => no_extension,
        ModuleSpecifierEnding::JsExtension => {
            format!(
                "{no_extension}{}",
                get_js_extension_for_file(file_name, options)
            )
        }
        ModuleSpecifierEnding::TsExtension => {
            // For now, we don't know if this import is going to be type-only, which means we don't
            // know if a .d.ts extension is valid, so use no extension or a .js extension
            if tspath::is_declaration_file_name(file_name) {
                let extensionless_priority = allowed_endings.iter().position(|ending| {
                    matches!(
                        ending,
                        ModuleSpecifierEnding::Minimal | ModuleSpecifierEnding::Index
                    )
                });
                if extensionless_priority
                    .is_some_and(|extensionless| js_priority.is_none_or(|js| extensionless < js))
                {
                    return no_extension;
                }
                return format!(
                    "{no_extension}{}",
                    get_js_extension_for_file(file_name, options)
                );
            }
            file_name.to_string()
        }
    }
}

fn try_get_module_name_from_root_dirs(
    root_dirs: &[String],
    module_file_name: &str,
    source_directory: &str,
    allowed_endings: &[ModuleSpecifierEnding],
    compiler_options: &core::CompilerOptions,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
) -> String {
    let normalized_target_paths = crate::get_paths_relative_to_root_dirs(
        module_file_name,
        root_dirs,
        host.use_case_sensitive_file_names(),
    );
    if normalized_target_paths.is_empty() {
        return String::new();
    }

    let normalized_source_paths = crate::get_paths_relative_to_root_dirs(
        source_directory,
        root_dirs,
        host.use_case_sensitive_file_names(),
    );
    let mut shortest = String::new();
    let mut shortest_sep_count = 0;
    for source_path in &normalized_source_paths {
        for target_path in &normalized_target_paths {
            let candidate =
                crate::ensure_path_is_non_module_name(&tspath::get_relative_path_from_directory(
                    source_path,
                    target_path,
                    &tspath::ComparePathsOptions {
                        use_case_sensitive_file_names: host.use_case_sensitive_file_names(),
                        current_directory: host.current_directory(),
                    },
                ));
            let candidate_sep_count = candidate.matches('/').count();
            if shortest.is_empty() || candidate_sep_count < shortest_sep_count {
                shortest = candidate;
                shortest_sep_count = candidate_sep_count;
            }
        }
    }

    if shortest.is_empty() {
        return String::new();
    }
    process_ending(&shortest, allowed_endings, compiler_options, Some(host))
}

#[derive(Clone, Debug, Default)]
pub(crate) struct NodeModuleSpecifierOptions {
    pub user_preferences: UserPreferences,
    pub package_name_only: bool,
    pub override_mode: core::ResolutionMode,
}

pub(crate) fn try_get_module_name_as_node_module(
    path_obj: &crate::ModulePath,
    info: &Info,
    importing_source_file: &impl SourceFileForSpecifierGeneration,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    options: &core::CompilerOptions,
    node_module_options: NodeModuleSpecifierOptions,
) -> String {
    let NodeModuleSpecifierOptions {
        user_preferences,
        package_name_only,
        override_mode,
    } = node_module_options;

    let Some(parts) = crate::get_node_module_path_parts(&path_obj.file_name) else {
        return String::new();
    };
    // Simplify the full file path to something that can be resolved by Node.
    let preferences = crate::get_module_specifier_preferences(
        &user_preferences,
        host,
        options,
        importing_source_file,
        "",
    );
    let allowed_endings = preferences.allowed_endings_in_preferred_order;

    let case_sensitive = host.use_case_sensitive_file_names();
    let mut module_specifier = path_obj.file_name.clone();
    let mut is_package_root_path = false;
    if !package_name_only {
        let mut package_root_index = parts.package_root_index;
        let mut module_file_name = String::new();
        loop {
            let pkg_json_results = try_directory_with_package_json(
                parts,
                path_obj,
                importing_source_file,
                host,
                DirectoryWithPackageJsonOptions {
                    override_mode,
                    options,
                    allowed_endings: &allowed_endings,
                    root_idx: package_root_index,
                },
            );
            if pkg_json_results.blocked_by_exports {
                return String::new();
            }
            if pkg_json_results.verbatim_from_exports {
                return pkg_json_results.module_file_to_try;
            }
            if !pkg_json_results.package_root_path.is_empty() {
                module_specifier = pkg_json_results.package_root_path;
                is_package_root_path = true;
                break;
            }
            if module_file_name.is_empty() {
                module_file_name = pkg_json_results.module_file_to_try;
            }
            if package_root_index == -1 {
                module_specifier =
                    process_ending(&module_file_name, &allowed_endings, options, Some(host));
                break;
            }
            let next = core::index_after(
                &path_obj.file_name,
                "/",
                (package_root_index + 1).max(0) as usize,
            );
            if next == -1 {
                module_specifier =
                    process_ending(&module_file_name, &allowed_endings, options, Some(host));
                break;
            }
            package_root_index = next;
        }
    }

    if path_obj.is_redirect && !is_package_root_path {
        return String::new();
    }

    let global_typings_cache_location = host.global_typings_cache_location();
    let path_to_top_level_node_modules = &module_specifier[..parts.top_level_node_modules_index];
    if !has_prefix(
        &info.source_directory,
        path_to_top_level_node_modules,
        case_sensitive,
    ) || !global_typings_cache_location.is_empty()
        && has_prefix(
            &global_typings_cache_location,
            path_to_top_level_node_modules,
            case_sensitive,
        )
    {
        return String::new();
    }

    let node_modules_directory_name = &module_specifier[parts.top_level_package_name_index + 1..];
    ts_module::get_package_name_from_types_package_name(node_modules_directory_name)
}

#[derive(Default)]
struct PkgJsonDirAttemptResult {
    module_file_to_try: String,
    package_root_path: String,
    blocked_by_exports: bool,
    verbatim_from_exports: bool,
}

struct DirectoryWithPackageJsonOptions<'a> {
    override_mode: core::ResolutionMode,
    options: &'a core::CompilerOptions,
    allowed_endings: &'a [ModuleSpecifierEnding],
    root_idx: isize,
}

fn try_directory_with_package_json(
    parts: crate::NodeModulePathParts,
    path_obj: &crate::ModulePath,
    importing_source_file: &impl SourceFileForSpecifierGeneration,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    directory_options: DirectoryWithPackageJsonOptions<'_>,
) -> PkgJsonDirAttemptResult {
    let DirectoryWithPackageJsonOptions {
        override_mode,
        options,
        allowed_endings,
        root_idx,
    } = directory_options;

    let mut root_idx = root_idx;
    if root_idx == -1 {
        root_idx = path_obj.file_name.len() as isize; // TODO: possible strada bug? -1 in js slice removes characters from the end, in go it panics - js behavior seems unwanted here?
    }
    let package_root_path = path_obj.file_name[..root_idx as usize].to_string();
    let package_json_path = tspath::combine_paths(&package_root_path, &["package.json"]);
    let mut module_file_to_try = path_obj.file_name.clone();
    let mut maybe_blocked_by_types_versions = false;
    let package_json = host.package_json_info(&package_json_path);
    if package_json.is_none() {
        let file_name = path_obj
            .file_name
            .get((parts.package_root_index + 1).max(0) as usize..)
            .unwrap_or_default();
        if matches!(
            file_name,
            "index.d.ts" | "index.js" | "index.ts" | "index.tsx"
        ) {
            return PkgJsonDirAttemptResult {
                module_file_to_try,
                package_root_path,
                ..Default::default()
            };
        }
        return PkgJsonDirAttemptResult {
            module_file_to_try,
            ..Default::default()
        };
    }
    let package_json = package_json.unwrap();
    let mut import_mode = if override_mode == core::RESOLUTION_MODE_NONE {
        host.default_resolution_mode_for_file(&FileNameOnly {
            file_name: importing_source_file.file_name(),
            path: importing_source_file.path(),
        })
    } else {
        override_mode
    };

    let package_json_content = package_json.get_contents();
    if options.get_resolve_package_json_exports() {
        let node_modules_directory_name =
            &package_root_path[parts.top_level_package_name_index + 1..];
        let package_name =
            ts_module::get_package_name_from_types_package_name(node_modules_directory_name);

        if tspath::file_extension_is_one_of(
            &path_obj.file_name,
            &[
                tspath::EXTENSION_CJS,
                tspath::EXTENSION_CTS,
                tspath::EXTENSION_DCTS,
            ],
        ) {
            import_mode = core::RESOLUTION_MODE_COMMON_JS;
        } else if tspath::file_extension_is_one_of(
            &path_obj.file_name,
            &[
                tspath::EXTENSION_MJS,
                tspath::EXTENSION_MTS,
                tspath::EXTENSION_DMTS,
            ],
        ) {
            import_mode = core::RESOLUTION_MODE_ESM;
        }

        let conditions = ts_module::get_conditions(options, import_mode);
        if let Some(content) = package_json_content {
            let mut exports = content.fields.path_fields.exports.clone();
            if exports.json_value.type_ != packagejson::JsonValueType::NotPresent {
                let from_exports = try_get_module_name_from_exports(
                    options,
                    host,
                    &path_obj.file_name,
                    &package_root_path,
                    &package_name,
                    &mut exports,
                    &conditions,
                );
                if !from_exports.is_empty() {
                    return PkgJsonDirAttemptResult {
                        module_file_to_try: from_exports,
                        verbatim_from_exports: true,
                        ..Default::default()
                    };
                }
                return PkgJsonDirAttemptResult {
                    module_file_to_try: path_obj.file_name.clone(),
                    blocked_by_exports: true,
                    ..Default::default()
                };
            }
        }
    }

    let mut version_paths = packagejson::VersionPaths::default();
    if let Some(content) = package_json_content
        && content.fields.path_fields.types_versions.type_ == packagejson::JsonValueType::Object
    {
        version_paths = content.get_version_paths(None::<fn(&ts_diagnostics::Message, &[String])>);
    }
    if let Some(paths) = version_paths.get_paths() {
        let sub_module_name = &path_obj.file_name[package_root_path.len() + 1..];
        let from_paths = try_get_module_name_from_paths(
            sub_module_name,
            paths,
            allowed_endings,
            &package_root_path,
            host,
            options,
        );
        if from_paths.is_empty() {
            maybe_blocked_by_types_versions = true;
        } else {
            module_file_to_try = tspath::combine_paths(&package_root_path, &[&from_paths]);
        }
    }

    let mut main_file_relative = "index.js".to_string();
    if let Some(content) = package_json_content {
        if content.fields.path_fields.typings.valid {
            main_file_relative = content.fields.path_fields.typings.value.clone();
        } else if content.fields.path_fields.types.valid {
            main_file_relative = content.fields.path_fields.types.value.clone();
        } else if content.fields.path_fields.main.valid {
            main_file_relative = content.fields.path_fields.main.value.clone();
        }
    }

    if !(main_file_relative.is_empty()
        || maybe_blocked_by_types_versions
            && version_paths
                .get_paths()
                .is_some_and(|paths| matches_pattern_or_exact(paths, &main_file_relative)))
    {
        let main_export_file = tspath::to_path(
            &main_file_relative,
            &package_root_path,
            host.use_case_sensitive_file_names(),
        );
        let compare_opt = tspath::ComparePathsOptions {
            use_case_sensitive_file_names: host.use_case_sensitive_file_names(),
            current_directory: host.current_directory(),
        };
        let matches_main_export = tspath::compare_paths(
            &tspath::remove_file_extension(&main_export_file),
            &tspath::remove_file_extension(&module_file_to_try),
            &compare_opt,
        ) == std::cmp::Ordering::Equal;
        let matches_index_fallback = package_json_content.is_none_or(|content| {
            content.fields.header_fields.type_.value != "module"
                && !tspath::file_extension_is_one_of(
                    &module_file_to_try,
                    tspath::EXTENSIONS_NOT_SUPPORTING_EXTENSIONLESS_RESOLUTION,
                )
                && has_prefix(
                    &module_file_to_try,
                    &main_export_file,
                    host.use_case_sensitive_file_names(),
                )
                && tspath::compare_paths(
                    &tspath::get_directory_path(&module_file_to_try),
                    tspath::remove_trailing_directory_separator(&main_export_file),
                    &compare_opt,
                ) == std::cmp::Ordering::Equal
                && tspath::remove_file_extension(&tspath::get_base_file_name(&module_file_to_try))
                    == "index"
        });
        if matches_main_export || matches_index_fallback {
            return PkgJsonDirAttemptResult {
                package_root_path,
                module_file_to_try,
                ..Default::default()
            };
        }
    }

    PkgJsonDirAttemptResult {
        module_file_to_try,
        ..Default::default()
    }
}

fn try_get_module_name_from_exports(
    options: &core::CompilerOptions,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    target_file_path: &str,
    package_directory: &str,
    package_name: &str,
    exports: &mut packagejson::ExportsOrImports,
    conditions: &[String],
) -> String {
    if exports.is_subpaths() {
        for (k, subk) in exports.as_object() {
            let sub_package_name = tspath::get_normalized_absolute_path(
                &tspath::combine_paths(package_name, &[k]),
                "",
            );
            let mode = if k.ends_with('/') {
                MatchingMode::Directory
            } else if k.contains('*') {
                MatchingMode::Pattern
            } else {
                MatchingMode::Exact
            };
            let result = try_get_module_name_from_exports_or_imports(
                options,
                host,
                ExportsOrImportsModuleNameInput {
                    target_file_path,
                    package_directory,
                    package_name: &sub_package_name,
                    exports: subk,
                    conditions,
                    mode,
                    is_imports: false,
                    prefer_ts_extension: false,
                },
            );
            if !result.is_empty() {
                return result;
            }
        }
    }
    try_get_module_name_from_exports_or_imports(
        options,
        host,
        ExportsOrImportsModuleNameInput {
            target_file_path,
            package_directory,
            package_name,
            exports: &exports.json_value.value,
            conditions,
            mode: MatchingMode::Exact,
            is_imports: false,
            prefer_ts_extension: false,
        },
    )
}

fn try_get_module_name_from_package_json_imports(
    module_file_name: &str,
    source_directory: &str,
    options: &core::CompilerOptions,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    import_mode: core::ResolutionMode,
    prefer_ts_extension: bool,
) -> String {
    if !options.get_resolve_package_json_imports() {
        return String::new();
    }

    let ancestor_directory_with_package_json =
        host.nearest_ancestor_directory_with_package_json(source_directory);
    if ancestor_directory_with_package_json.is_empty() {
        return String::new();
    }
    let package_json_path =
        tspath::combine_paths(&ancestor_directory_with_package_json, &["package.json"]);

    let Some(info) = host.package_json_info(&package_json_path) else {
        return String::new();
    };
    let Some(contents) = info.get_contents() else {
        return String::new();
    };

    let imports = &contents.fields.path_fields.imports;
    match imports.json_value.type_ {
        packagejson::JsonValueType::NotPresent
        | packagejson::JsonValueType::Array
        | packagejson::JsonValueType::String => String::new(),
        packagejson::JsonValueType::Object => {
            let conditions = ts_module::get_conditions(options, import_mode);
            for (k, value) in imports.as_object() {
                if k == "#" || k == "#/" || !k.starts_with('#') {
                    continue;
                }
                if k.starts_with("#/")
                    && options.get_module_resolution_kind() != core::ModuleResolutionKind::NodeNext
                    && options.get_module_resolution_kind() != core::ModuleResolutionKind::Bundler
                {
                    continue;
                }
                let mode = if k.ends_with('/') {
                    MatchingMode::Directory
                } else if k.contains('*') {
                    MatchingMode::Pattern
                } else {
                    MatchingMode::Exact
                };
                let result = try_get_module_name_from_exports_or_imports(
                    options,
                    host,
                    ExportsOrImportsModuleNameInput {
                        target_file_path: module_file_name,
                        package_directory: &ancestor_directory_with_package_json,
                        package_name: k,
                        exports: value,
                        conditions: &conditions,
                        mode,
                        is_imports: true,
                        prefer_ts_extension,
                    },
                );
                if !result.is_empty() {
                    return result;
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

#[derive(Clone)]
struct SpecPair {
    ending: ModuleSpecifierEnding,
    value: String,
}

fn try_get_module_name_from_paths(
    relative_to_base_url: &str,
    paths: &ts_collections::OrderedMap<String, Vec<String>>,
    allowed_endings: &[ModuleSpecifierEnding],
    base_directory: &str,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    compiler_options: &core::CompilerOptions,
) -> String {
    let case_sensitive = host.use_case_sensitive_file_names();
    for (key, values) in paths.entries() {
        for pattern_text in values {
            let normalized = tspath::normalize_path(pattern_text);
            let mut pattern =
                get_relative_path_if_in_same_volume(&normalized, base_directory, case_sensitive);
            if pattern.is_empty() {
                pattern = normalized;
            }
            let (prefix, suffix, has_star) = if let Some((prefix, suffix)) = pattern.split_once('*')
            {
                (prefix, suffix, true)
            } else {
                (pattern.as_str(), "", false)
            };

            let mut candidates = Vec::new();
            for ending in allowed_endings {
                let result = process_ending(
                    relative_to_base_url,
                    &[*ending],
                    compiler_options,
                    None::<&NoHost>,
                );
                candidates.push(SpecPair {
                    ending: *ending,
                    value: result,
                });
            }
            if !tspath::try_get_extension_from_path(&pattern).is_empty() {
                candidates.push(SpecPair {
                    ending: ModuleSpecifierEnding::JsExtension,
                    value: relative_to_base_url.to_string(),
                });
            }

            if has_star {
                for candidate in candidates {
                    if candidate.value.len() >= prefix.len() + suffix.len()
                        && has_prefix(&candidate.value, prefix, case_sensitive)
                        && has_suffix(&candidate.value, suffix, case_sensitive)
                        && validate_ending(&candidate, relative_to_base_url, compiler_options, host)
                    {
                        let matched_star =
                            &candidate.value[prefix.len()..candidate.value.len() - suffix.len()];
                        if !tspath::path_is_relative(matched_star) {
                            return replace_first_star(key, matched_star);
                        }
                    }
                }
            } else if candidates.iter().any(|candidate| {
                candidate.ending != ModuleSpecifierEnding::Minimal && pattern == candidate.value
            }) || candidates.iter().any(|candidate| {
                candidate.ending == ModuleSpecifierEnding::Minimal
                    && pattern == candidate.value
                    && validate_ending(candidate, relative_to_base_url, compiler_options, host)
            }) {
                return key.to_string();
            }
        }
    }
    String::new()
}

fn validate_ending(
    candidate: &SpecPair,
    relative_to_base_url: &str,
    compiler_options: &core::CompilerOptions,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
) -> bool {
    candidate.ending != ModuleSpecifierEnding::Minimal
        || candidate.value
            == process_ending(
                relative_to_base_url,
                &[candidate.ending],
                compiler_options,
                Some(host),
            )
}

pub(crate) struct ExportsOrImportsModuleNameInput<'a> {
    pub target_file_path: &'a str,
    pub package_directory: &'a str,
    pub package_name: &'a str,
    pub exports: &'a serde_json::Value,
    pub conditions: &'a [String],
    pub mode: MatchingMode,
    pub is_imports: bool,
    pub prefer_ts_extension: bool,
}

pub(crate) fn try_get_module_name_from_exports_or_imports(
    options: &core::CompilerOptions,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    input: ExportsOrImportsModuleNameInput<'_>,
) -> String {
    let ExportsOrImportsModuleNameInput {
        target_file_path,
        package_directory,
        package_name,
        exports,
        conditions,
        mode,
        is_imports,
        prefer_ts_extension,
    } = input;

    match exports {
        serde_json::Value::String(str_value) => {
            let mut output_file = String::new();
            let mut declaration_file = String::new();
            if is_imports {
                output_file = get_output_js_file_name_worker(target_file_path, options, host);
                declaration_file =
                    get_output_declaration_file_name_worker(target_file_path, options, host);
            }
            let path_or_pattern = tspath::get_normalized_absolute_path(
                &tspath::combine_paths(package_directory, &[str_value]),
                "",
            );
            let extension_swapped_target = if tspath::has_ts_file_extension(target_file_path) {
                let js_extension = ts_module::try_get_js_extension_for_file(
                    target_file_path,
                    options.jsx == core::JsxEmit::Preserve,
                );
                if js_extension.is_empty() {
                    String::new()
                } else {
                    format!(
                        "{}{}",
                        tspath::remove_file_extension(target_file_path),
                        js_extension
                    )
                }
            } else {
                String::new()
            };
            let can_try_ts_extension = prefer_ts_extension
                && tspath::has_implementation_ts_file_extension(target_file_path);
            let compare_opts = tspath::ComparePathsOptions {
                use_case_sensitive_file_names: host.use_case_sensitive_file_names(),
                current_directory: host.current_directory(),
            };

            match mode {
                MatchingMode::Exact => {
                    if !extension_swapped_target.is_empty()
                        && tspath::compare_paths(
                            &extension_swapped_target,
                            &path_or_pattern,
                            &compare_opts,
                        ) == std::cmp::Ordering::Equal
                        || tspath::compare_paths(target_file_path, &path_or_pattern, &compare_opts)
                            == std::cmp::Ordering::Equal
                        || !output_file.is_empty()
                            && tspath::compare_paths(&output_file, &path_or_pattern, &compare_opts)
                                == std::cmp::Ordering::Equal
                        || !declaration_file.is_empty()
                            && tspath::compare_paths(
                                &declaration_file,
                                &path_or_pattern,
                                &compare_opts,
                            ) == std::cmp::Ordering::Equal
                    {
                        return package_name.to_string();
                    }
                }
                MatchingMode::Directory => {
                    if can_try_ts_extension
                        && tspath::contains_path(&path_or_pattern, target_file_path, &compare_opts)
                    {
                        let fragment = tspath::get_relative_path_from_directory(
                            &path_or_pattern,
                            target_file_path,
                            &compare_opts,
                        );
                        return tspath::get_normalized_absolute_path(
                            &tspath::combine_paths(
                                &tspath::combine_paths(package_name, &[str_value]),
                                &[&fragment],
                            ),
                            "",
                        );
                    }
                    if !extension_swapped_target.is_empty()
                        && tspath::contains_path(
                            &path_or_pattern,
                            &extension_swapped_target,
                            &compare_opts,
                        )
                    {
                        let fragment = tspath::get_relative_path_from_directory(
                            &path_or_pattern,
                            &extension_swapped_target,
                            &compare_opts,
                        );
                        return tspath::get_normalized_absolute_path(
                            &tspath::combine_paths(
                                &tspath::combine_paths(package_name, &[str_value]),
                                &[&fragment],
                            ),
                            "",
                        );
                    }
                    if !can_try_ts_extension
                        && tspath::contains_path(&path_or_pattern, target_file_path, &compare_opts)
                    {
                        let fragment = tspath::get_relative_path_from_directory(
                            &path_or_pattern,
                            target_file_path,
                            &compare_opts,
                        );
                        return tspath::get_normalized_absolute_path(
                            &tspath::combine_paths(
                                &tspath::combine_paths(package_name, &[str_value]),
                                &[&fragment],
                            ),
                            "",
                        );
                    }
                    if !output_file.is_empty()
                        && tspath::contains_path(&path_or_pattern, &output_file, &compare_opts)
                    {
                        let fragment = tspath::get_relative_path_from_directory(
                            &path_or_pattern,
                            &output_file,
                            &compare_opts,
                        );
                        return tspath::combine_paths(package_name, &[&fragment]);
                    }
                    if !declaration_file.is_empty()
                        && tspath::contains_path(&path_or_pattern, &declaration_file, &compare_opts)
                    {
                        let fragment = tspath::get_relative_path_from_directory(
                            &path_or_pattern,
                            &declaration_file,
                            &compare_opts,
                        );
                        let js_extension = get_js_extension_for_file(&declaration_file, options);
                        let fragment_with_js_extension =
                            tspath::change_extension(&fragment, &js_extension);
                        return tspath::combine_paths(package_name, &[&fragment_with_js_extension]);
                    }
                }
                MatchingMode::Pattern => {
                    let Some((leading_slice, trailing_slice)) = path_or_pattern.split_once('*')
                    else {
                        return String::new();
                    };
                    let case_sensitive = host.use_case_sensitive_file_names();
                    if can_try_ts_extension
                        && has_prefix_and_suffix_without_overlap(
                            target_file_path,
                            leading_slice,
                            trailing_slice,
                            case_sensitive,
                        )
                    {
                        let star_replacement = &target_file_path
                            [leading_slice.len()..target_file_path.len() - trailing_slice.len()];
                        return replace_first_star(package_name, star_replacement);
                    }
                    if !extension_swapped_target.is_empty()
                        && has_prefix_and_suffix_without_overlap(
                            &extension_swapped_target,
                            leading_slice,
                            trailing_slice,
                            case_sensitive,
                        )
                    {
                        let star_replacement = &extension_swapped_target[leading_slice.len()
                            ..extension_swapped_target.len() - trailing_slice.len()];
                        return replace_first_star(package_name, star_replacement);
                    }
                    if !can_try_ts_extension
                        && has_prefix_and_suffix_without_overlap(
                            target_file_path,
                            leading_slice,
                            trailing_slice,
                            case_sensitive,
                        )
                    {
                        let star_replacement = &target_file_path
                            [leading_slice.len()..target_file_path.len() - trailing_slice.len()];
                        return replace_first_star(package_name, star_replacement);
                    }
                    if !output_file.is_empty()
                        && has_prefix_and_suffix_without_overlap(
                            &output_file,
                            leading_slice,
                            trailing_slice,
                            case_sensitive,
                        )
                    {
                        let star_replacement = &output_file
                            [leading_slice.len()..output_file.len() - trailing_slice.len()];
                        return replace_first_star(package_name, star_replacement);
                    }
                    if !declaration_file.is_empty()
                        && has_prefix_and_suffix_without_overlap(
                            &declaration_file,
                            leading_slice,
                            trailing_slice,
                            case_sensitive,
                        )
                    {
                        let star_replacement = &declaration_file
                            [leading_slice.len()..declaration_file.len() - trailing_slice.len()];
                        let substituted = replace_first_star(package_name, star_replacement);
                        let js_extension = get_js_extension_for_file(&declaration_file, options);
                        if !js_extension.is_empty() {
                            return tspath::change_full_extension(&substituted, &js_extension);
                        }
                    }
                }
            }
            String::new()
        }
        serde_json::Value::Array(arr) => {
            for entry in arr {
                let result = try_get_module_name_from_exports_or_imports(
                    options,
                    host,
                    ExportsOrImportsModuleNameInput {
                        target_file_path,
                        package_directory,
                        package_name,
                        exports: entry,
                        conditions,
                        mode,
                        is_imports,
                        prefer_ts_extension,
                    },
                );
                if !result.is_empty() {
                    return result;
                }
            }
            String::new()
        }
        serde_json::Value::Object(obj) => {
            for (key, value) in obj {
                if key == "default"
                    || conditions.contains(key)
                    || conditions.iter().any(|condition| condition == "types")
                        && ts_module::is_applicable_versioned_types_key(key)
                {
                    let result = try_get_module_name_from_exports_or_imports(
                        options,
                        host,
                        ExportsOrImportsModuleNameInput {
                            target_file_path,
                            package_directory,
                            package_name,
                            exports: value,
                            conditions,
                            mode,
                            is_imports,
                            prefer_ts_extension,
                        },
                    );
                    if !result.is_empty() {
                        return result;
                    }
                }
            }
            String::new()
        }
        serde_json::Value::Null => String::new(),
        _ => String::new(),
    }
}

// `importingSourceFile` and `importingSourceFileName`? Why not just use `importingSourceFile.path`?
// Because when this is called by the declaration emitter, `importingSourceFile` is the implementation
// file, but `importingSourceFileName` and `toFileName` refer to declaration files (the former to the
// one currently being produced; the latter to the one being imported). We need an implementation file
// just to get its `impliedNodeFormat` and to detect certain preferences from existing import module
// specifiers.
pub fn get_module_specifier(
    compiler_options: &core::CompilerOptions,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    importing_source_file: &impl SourceFileForSpecifierGeneration,
    importing_source_file_name: &str,
    old_import_specifier: &str,
    to_file_name: &str,
    options: ModuleSpecifierOptions,
) -> String {
    get_module_specifier_with_preferences(
        compiler_options,
        host,
        importing_source_file,
        importing_source_file_name,
        old_import_specifier,
        to_file_name,
        UpdateModuleSpecifierOptions {
            user_preferences: UserPreferences::default(),
            options,
        },
    )
}

#[derive(Clone, Debug, Default)]
pub struct UpdateModuleSpecifierOptions {
    pub user_preferences: UserPreferences,
    pub options: ModuleSpecifierOptions,
}

pub fn update_module_specifier(
    compiler_options: &core::CompilerOptions,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    importing_source_file: &impl SourceFileForSpecifierGeneration,
    importing_source_file_name: &str,
    old_import_specifier: &str,
    to_file_name: &str,
    update_options: UpdateModuleSpecifierOptions,
) -> String {
    get_module_specifier_with_preferences(
        compiler_options,
        host,
        importing_source_file,
        importing_source_file_name,
        old_import_specifier,
        to_file_name,
        update_options,
    )
}

fn get_module_specifier_with_preferences(
    compiler_options: &core::CompilerOptions,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    importing_source_file: &impl SourceFileForSpecifierGeneration,
    importing_source_file_name: &str,
    old_import_specifier: &str,
    to_file_name: &str,
    update_options: UpdateModuleSpecifierOptions,
) -> String {
    let UpdateModuleSpecifierOptions {
        user_preferences,
        options,
    } = update_options;

    let info = get_info(importing_source_file_name, host);
    let module_paths = get_all_module_paths(
        info.clone(),
        to_file_name,
        host,
        compiler_options,
        user_preferences.clone(),
        options,
    );
    let preferences = crate::get_module_specifier_preferences(
        &user_preferences,
        host,
        compiler_options,
        importing_source_file,
        old_import_specifier,
    );

    let resolution_mode = if options.override_import_mode == core::RESOLUTION_MODE_NONE {
        host.default_resolution_mode_for_file(&FileNameOnly {
            file_name: importing_source_file.file_name(),
            path: importing_source_file.path(),
        })
    } else {
        options.override_import_mode
    };

    for module_path in &module_paths {
        let first_defined = try_get_module_name_as_node_module(
            module_path,
            &info,
            importing_source_file,
            host,
            compiler_options,
            NodeModuleSpecifierOptions {
                user_preferences: user_preferences.clone(),
                package_name_only: false,
                override_mode: options.override_import_mode,
            },
        );
        if !first_defined.is_empty() {
            return first_defined;
        }
    }

    get_local_module_specifier_worker(
        to_file_name,
        &info,
        compiler_options,
        host,
        resolution_mode,
        &preferences,
        false,
    )
}

struct SimpleSourceFileForSpecifierGeneration {
    file_name: String,
}

impl SourceFileForSpecifierGeneration for SimpleSourceFileForSpecifierGeneration {
    fn path(&self) -> tspath::Path {
        self.file_name.clone()
    }

    fn file_name(&self) -> String {
        self.file_name.clone()
    }

    fn imports(&self) -> Vec<ast::StringLiteralLike> {
        Vec::new()
    }

    fn import_text(&self, _import: &ast::StringLiteralLike) -> String {
        unreachable!("SimpleSourceFileForSpecifierGeneration never exposes import nodes")
    }

    fn is_js(&self) -> bool {
        false
    }
}

struct FileNameOnly {
    file_name: String,
    path: tspath::Path,
}

impl ast::HasFileName for FileNameOnly {
    fn file_name(&self) -> String {
        self.file_name.clone()
    }

    fn path(&self) -> tspath::Path {
        self.path.clone()
    }
}

#[derive(Default)]
struct NoHost;

impl ModuleSpecifierGenerationHost for NoHost {
    fn symlink_cache(&self) -> Option<ts_symlinks::KnownSymlinks> {
        None
    }

    fn common_source_directory(&self) -> String {
        String::new()
    }

    fn global_typings_cache_location(&self) -> String {
        String::new()
    }

    fn use_case_sensitive_file_names(&self) -> bool {
        true
    }

    fn current_directory(&self) -> String {
        String::new()
    }

    fn project_reference_from_source(
        &self,
        _path: tspath::Path,
    ) -> Option<ts_tsoptions::SourceOutputAndProjectReference> {
        None
    }

    fn redirect_targets(&self, _path: tspath::Path) -> Vec<String> {
        Vec::new()
    }

    fn source_of_project_reference_if_output_included(
        &self,
        file: &dyn ast::HasFileName,
    ) -> String {
        file.file_name()
    }

    fn file_exists(&self, _path: &str) -> bool {
        false
    }

    fn nearest_ancestor_directory_with_package_json(&self, _dirname: &str) -> String {
        String::new()
    }

    fn package_json_info(&self, _pkg_json_path: &str) -> Option<packagejson::InfoCacheEntry> {
        None
    }

    fn default_resolution_mode_for_file(
        &self,
        _file: &dyn ast::HasFileName,
    ) -> core::ResolutionMode {
        core::RESOLUTION_MODE_NONE
    }

    fn resolved_module_from_module_specifier(
        &self,
        _file: &dyn ast::HasFileName,
        _module_specifier: &ast::StringLiteralLike,
    ) -> Option<ts_module::ResolvedModule> {
        None
    }

    fn mode_for_usage_location(
        &self,
        file: &dyn ast::HasFileName,
        _module_specifier: &ast::StringLiteralLike,
    ) -> core::ResolutionMode {
        self.default_resolution_mode_for_file(file)
    }
}

fn get_js_extension_for_file(file_name: &str, options: &core::CompilerOptions) -> String {
    let result =
        ts_module::try_get_js_extension_for_file(file_name, options.jsx == core::JsxEmit::Preserve);
    if result.is_empty() {
        panic!(
            "Extension {} is unsupported:: FileName:: {}",
            crate::extension_from_path(file_name),
            file_name
        );
    }
    result
}

fn try_get_any_file_from_path_for_host(
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    path: &str,
) -> bool {
    let current_directory = host.current_directory();
    crate::try_get_any_file_from_path(path, |candidate| {
        host.file_exists(&tspath::get_normalized_absolute_path(
            candidate,
            &current_directory,
        ))
    })
}

fn matches_pattern_or_exact(
    paths: &ts_collections::OrderedMap<String, Vec<String>>,
    candidate: &str,
) -> bool {
    for (key, _) in paths.entries() {
        if key == candidate {
            return true;
        }
        if let Some((prefix, suffix)) = key.split_once('*')
            && candidate.len() >= prefix.len() + suffix.len()
            && candidate.starts_with(prefix)
            && candidate.ends_with(suffix)
        {
            return true;
        }
    }
    false
}

fn get_output_js_file_name_worker(
    input_file_name: &str,
    options: &core::CompilerOptions,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
) -> String {
    tspath::change_extension(
        &get_output_path_without_changing_extension(input_file_name, &options.out_dir, host),
        &get_output_extension(input_file_name, options),
    )
}

fn get_output_declaration_file_name_worker(
    input_file_name: &str,
    options: &core::CompilerOptions,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
) -> String {
    let dir = if options.declaration_dir.is_empty() {
        &options.out_dir
    } else {
        &options.declaration_dir
    };
    tspath::change_extension(
        &get_output_path_without_changing_extension(input_file_name, dir, host),
        &tspath::get_declaration_emit_extension_for_path(input_file_name),
    )
}

fn get_output_path_without_changing_extension(
    input_file_name: &str,
    output_dir: &str,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
) -> String {
    if output_dir.is_empty() {
        return input_file_name.to_string();
    }
    let relative = tspath::get_relative_path_from_directory(
        &host.common_source_directory(),
        input_file_name,
        &tspath::ComparePathsOptions {
            use_case_sensitive_file_names: host.use_case_sensitive_file_names(),
            current_directory: host.current_directory(),
        },
    );
    tspath::combine_paths(output_dir, &[&relative])
}

fn get_output_extension(file_name: &str, options: &core::CompilerOptions) -> String {
    if tspath::file_extension_is(file_name, tspath::EXTENSION_JSON) {
        return tspath::EXTENSION_JSON.to_string();
    }
    if options.jsx == core::JsxEmit::Preserve
        && tspath::file_extension_is_one_of(
            file_name,
            &[tspath::EXTENSION_JSX, tspath::EXTENSION_TSX],
        )
    {
        return tspath::EXTENSION_JSX.to_string();
    }
    if tspath::file_extension_is_one_of(file_name, &[tspath::EXTENSION_MTS, tspath::EXTENSION_MJS])
    {
        return tspath::EXTENSION_MJS.to_string();
    }
    if tspath::file_extension_is_one_of(file_name, &[tspath::EXTENSION_CTS, tspath::EXTENSION_CJS])
    {
        return tspath::EXTENSION_CJS.to_string();
    }
    tspath::EXTENSION_JS.to_string()
}

fn has_prefix(s: &str, prefix: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        s.starts_with(prefix)
    } else {
        s.to_lowercase().starts_with(&prefix.to_lowercase())
    }
}

fn has_suffix(s: &str, suffix: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        s.ends_with(suffix)
    } else {
        s.to_lowercase().ends_with(&suffix.to_lowercase())
    }
}

fn has_prefix_and_suffix_without_overlap(
    s: &str,
    prefix: &str,
    suffix: &str,
    case_sensitive: bool,
) -> bool {
    s.len() >= prefix.len() + suffix.len()
        && has_prefix(s, prefix, case_sensitive)
        && has_suffix(s, suffix, case_sensitive)
}
