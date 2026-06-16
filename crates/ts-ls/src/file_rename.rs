use ts_ast as ast;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_lsproto::DocumentUriExt;
use ts_modulespecifiers as modulespecifiers;
use ts_scanner as scanner;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::LanguageService;
use crate::change;
use crate::lsconv;

type PathUpdater = Box<dyn Fn(&str) -> (String, bool)>;

#[derive(Clone, Debug, Default)]
pub struct ToImport {
    pub new_file_name: String,
    pub updated: bool,
}

#[derive(Clone, Debug, Default)]
struct ImportingSourceFileForSpecifierGeneration {
    path: String,
    file_name: String,
    imports: Vec<ast::StringLiteralLike>,
    import_text: std::collections::HashMap<ast::Node, String>,
    is_js: bool,
}

impl ImportingSourceFileForSpecifierGeneration {
    fn new(source_file: &ast::SourceFile, file_name: &str) -> Self {
        let import_text = source_file
            .imports()
            .iter()
            .map(|import| (*import, source_file.store().text(*import)))
            .collect();
        Self {
            path: source_file.path(),
            file_name: file_name.to_owned(),
            imports: source_file.imports().to_vec(),
            import_text,
            is_js: source_file.is_js(),
        }
    }
}

impl modulespecifiers::SourceFileForSpecifierGeneration
    for ImportingSourceFileForSpecifierGeneration
{
    fn path(&self) -> String {
        self.path.clone()
    }

    fn file_name(&self) -> String {
        self.file_name.clone()
    }

    fn imports(&self) -> Vec<ast::StringLiteralLike> {
        self.imports.clone()
    }

    fn import_text(&self, import: &ast::StringLiteralLike) -> String {
        self.import_text
            .get(import)
            .cloned()
            .expect("source-file import specifier text should be captured")
    }

    fn is_js(&self) -> bool {
        self.is_js
    }
}

impl LanguageService<'_> {
    pub fn get_edits_for_file_rename(
        &self,
        ctx: &core::Context,
        old_uri: lsproto::DocumentUri,
        new_uri: lsproto::DocumentUri,
    ) -> Result<Vec<lsproto::TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile>, core::Error>
    {
        let program = self.get_program();
        let old_path = old_uri.file_name();
        let new_path = new_uri.file_name();

        let old_to_new = self.create_path_updater(&old_path, &new_path);

        let mut change_tracker = change::new_tracker(
            ctx.clone(),
            program.options(),
            self.format_options(),
            &self.converters,
        );
        self.update_tsconfig_files(
            program,
            &mut change_tracker,
            &old_to_new,
            &old_path,
            &new_path,
        );
        self.update_imports_for_file_rename(ctx, program, &mut change_tracker, &old_to_new)?;

        let mut document_changes = Vec::new();

        // When renaming e.g. `foo.d.css.ts` -> `bar.d.css.ts`, also rename `foo.css` -> `bar.css` if it exists.
        if tspath::is_declaration_file_name(&old_path)
            && tspath::is_declaration_file_name(&new_path)
        {
            let dts_ext = tspath::get_declaration_file_extension(&old_path);
            let original_extensions =
                tspath::get_possible_original_input_extension_for_extension(&dts_ext);
            for ext in original_extensions {
                let old_original_path = tspath::change_full_extension(&old_path, &ext);
                if self
                    .host
                    .as_ref()
                    .is_some_and(|host| host.file_exists(&old_original_path))
                {
                    let new_dts_ext = tspath::get_declaration_file_extension(&old_path);
                    let new_original_extensions =
                        tspath::get_possible_original_input_extension_for_extension(&new_dts_ext);
                    if new_original_extensions.contains(&ext) {
                        let new_original_path = tspath::change_full_extension(&new_path, &ext);
                        document_changes.push(
                            lsproto::TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile {
                                rename_file: Some(lsproto::RenameFile {
                                    old_uri: lsconv::file_name_to_document_uri(&old_original_path),
                                    new_uri: lsconv::file_name_to_document_uri(&new_original_path),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                        );
                    }
                }
            }
        }

        for (file_name, edits) in change_tracker.get_changes() {
            let uri = lsconv::file_name_to_document_uri(&file_name);
            let lsp_edits = edits
                .into_iter()
                .map(
                    |edit| lsproto::TextEditOrAnnotatedTextEditOrSnippetTextEdit {
                        text_edit: Some(edit),
                        ..Default::default()
                    },
                )
                .collect();
            document_changes.push(
                lsproto::TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile {
                    text_document_edit: Some(lsproto::TextDocumentEdit {
                        text_document: lsproto::OptionalVersionedTextDocumentIdentifier {
                            uri,
                            ..Default::default()
                        },
                        edits: lsp_edits,
                    }),
                    ..Default::default()
                },
            );
        }

        Ok(document_changes)
    }

    pub fn create_path_updater(&self, old_path: &str, new_path: &str) -> PathUpdater {
        let use_case_sensitive_file_names = self.use_case_sensitive_file_names();
        let compare_options = tspath::ComparePathsOptions {
            use_case_sensitive_file_names,
            ..Default::default()
        };
        let old_path = old_path.to_owned();
        let new_path = new_path.to_owned();
        Box::new(move |path: &str| {
            if tspath::compare_paths(path, &old_path, &compare_options) == std::cmp::Ordering::Equal
            {
                return (new_path.clone(), true);
            }
            if tspath::starts_with_directory(path, &old_path, use_case_sensitive_file_names) {
                return (format!("{}{}", new_path, &path[old_path.len()..]), true);
            }
            (String::new(), false)
        })
    }

    pub fn update_tsconfig_files<'a>(
        &self,
        program: &'a compiler::Program,
        change_tracker: &mut change::Tracker<'a>,
        old_to_new: &PathUpdater,
        old_path: &str,
        new_path: &str,
    ) {
        let command_line = program.command_line();
        let Some(config_file) = command_line
            .config_file
            .as_ref()
            .map(|config_file| &config_file.source_file)
        else {
            return;
        };
        let config_dir = tspath::get_directory_path(&config_file.file_name());
        let Some(json_object_literal) = get_ts_config_object_literal_expression(config_file) else {
            return;
        };

        for_each_object_property(
            config_file.store(),
            json_object_literal,
            |property, property_name| match property_name.as_str() {
                "files" | "include" | "exclude" => {
                    let Some(initializer) = config_file.store().initializer(*property) else {
                        return;
                    };
                    let found_exact_match = update_paths_property(
                        config_file,
                        &config_dir,
                        property,
                        change_tracker,
                        old_to_new,
                        &self.converters,
                        self.use_case_sensitive_file_names(),
                    );
                    if found_exact_match
                        || property_name != "include"
                        || !ast::is_array_literal_expression(config_file.store(), initializer)
                    {
                        return;
                    }
                    let (old_spec, is_default) = command_line.get_matched_include_spec(old_path);
                    if !old_spec.is_empty() && !is_default {
                        let (new_spec, _) = command_line.get_matched_include_spec(new_path);
                        if new_spec.is_empty() {
                            if let Some(last) = config_file
                                .store()
                                .elements(initializer)
                                .and_then(|elements| elements.iter().last())
                            {
                                let new_node = change_tracker.node_factory.new_string_literal(
                                    relative_path_from_directory(
                                        &config_dir,
                                        new_path,
                                        self.use_case_sensitive_file_names(),
                                    ),
                                    ast::TOKEN_FLAGS_NONE,
                                );
                                change_tracker.insert_node_after(config_file, last, new_node);
                            }
                        }
                    }
                }
                "compilerOptions" => {
                    let Some(initializer) = config_file.store().initializer(*property) else {
                        return;
                    };
                    if !ast::is_object_literal_expression(config_file.store(), initializer) {
                        return;
                    }
                    for_each_object_property(
                        config_file.store(),
                        initializer,
                        |property, property_name| {
                            if let Some(option) =
                                tsoptions::COMMAND_LINE_COMPILER_OPTIONS_MAP.get(&property_name)
                            {
                                let element_option = option.elements();
                                if option.is_file_path
                                    || (option.kind
                                        == Some(tsoptions::COMMAND_LINE_OPTION_TYPE_LIST)
                                        && element_option
                                            .as_ref()
                                            .is_some_and(|element| element.is_file_path))
                                {
                                    update_paths_property(
                                        config_file,
                                        &config_dir,
                                        property,
                                        change_tracker,
                                        old_to_new,
                                        &self.converters,
                                        self.use_case_sensitive_file_names(),
                                    );
                                    return;
                                }
                            }

                            let Some(initializer) = config_file.store().initializer(*property)
                            else {
                                return;
                            };
                            if property_name != "paths"
                                || !ast::is_object_literal_expression(
                                    config_file.store(),
                                    initializer,
                                )
                            {
                                return;
                            }
                            for_each_object_property(
                                config_file.store(),
                                initializer,
                                |paths_property, _| {
                                    let Some(paths_initializer) =
                                        config_file.store().initializer(*paths_property)
                                    else {
                                        return;
                                    };
                                    if !ast::is_array_literal_expression(
                                        config_file.store(),
                                        paths_initializer,
                                    ) {
                                        return;
                                    }
                                    let elements: Vec<_> = config_file
                                        .store()
                                        .elements(paths_initializer)
                                        .map(|elements| elements.iter().collect())
                                        .unwrap_or_default();
                                    for element in elements {
                                        try_update_config_string(
                                            config_file,
                                            &config_dir,
                                            &element,
                                            change_tracker,
                                            old_to_new,
                                            &self.converters,
                                            self.use_case_sensitive_file_names(),
                                        );
                                    }
                                },
                            );
                        },
                    );
                }
                _ => {}
            },
        );
    }

    pub fn update_relative_path(
        &self,
        old_to_new: &PathUpdater,
        old_import_from_path: &str,
        new_import_from_path: &str,
        relative_specifier: &str,
    ) -> String {
        let old_absolute = tspath::normalize_path(&tspath::combine_paths(
            &tspath::get_directory_path(old_import_from_path),
            &[relative_specifier],
        ));
        let (new_absolute, ok) = old_to_new(&old_absolute);
        let new_absolute = if ok {
            new_absolute
        } else {
            old_absolute.clone()
        };
        relative_import_path_from_directory(
            &tspath::get_directory_path(new_import_from_path),
            &new_absolute,
            self.use_case_sensitive_file_names(),
        )
    }

    pub fn update_imports_for_file_rename<'a>(
        &self,
        ctx: &core::Context,
        program: &'a compiler::Program,
        change_tracker: &mut change::Tracker<'a>,
        old_to_new: &PathUpdater,
    ) -> Result<(), core::Error> {
        let all_files = program.get_parsed_source_files_refs();
        let module_specifier_preferences = self.user_preferences().module_specifier_preferences();

        for source_file in all_files {
            program.with_type_checker_for_file_using(
                compiler::CheckerAccess::context(ctx),
                source_file,
                |checker| {
                    let old_file_name = source_file.file_name();
                    let (new_from_old, file_moved) = old_to_new(source_file.file_name().as_str());
                    let mut new_import_from_path = source_file.file_name();
                    if file_moved {
                        new_import_from_path = new_from_old;
                    }

                    for reference in source_file.referenced_files() {
                        if !tspath::is_external_module_name_relative(&reference.file_name) {
                            continue;
                        }
                        let updated = self.update_relative_path(
                            old_to_new,
                            &old_file_name,
                            &new_import_from_path,
                            &reference.file_name,
                        );
                        if updated != reference.file_name {
                            change_tracker.replace_range_with_text(
                                source_file,
                                self.converters
                                    .to_lsp_range(source_file, reference.text_range),
                                &updated,
                            );
                        }
                    }

                    for import_string_literal in source_file.imports() {
                        let updated = self.get_updated_import_specifier(
                            program,
                            checker,
                            source_file,
                            import_string_literal,
                            old_to_new,
                            &new_import_from_path,
                            file_moved,
                            module_specifier_preferences.clone(),
                        );
                        if !updated.is_empty()
                            && updated != source_file.store().text(*import_string_literal)
                        {
                            change_tracker.replace_range_with_text(
                                source_file,
                                self.converters.to_lsp_range(
                                    source_file,
                                    create_string_text_range(source_file, import_string_literal),
                                ),
                                &updated,
                            );
                        }
                    }
                    Ok(())
                },
            )?;
        }
        Ok(())
    }

    // We assume the source file did not move to a different program.
    pub fn get_updated_import_specifier<'a>(
        &self,
        program: &'a compiler::Program,
        checker: &mut checker::Checker<'a, '_>,
        source_file: &'a ast::SourceFile,
        import_literal: &ast::StringLiteralLike,
        old_to_new: &PathUpdater,
        new_import_from_path: &str,
        importing_source_file_moved: bool,
        user_preferences: modulespecifiers::UserPreferences,
    ) -> String {
        let imported_module_symbol = checker.get_symbol_at_location_public(*import_literal);
        if is_ambient_module_symbol(checker, imported_module_symbol) {
            return String::new();
        }

        let target = get_source_file_to_import(program, source_file, import_literal, old_to_new);

        let Some(target) = target else {
            // First fall back: try every file in the program to see if any of them would match the import specifier, and if so, obtain the updated specifier for that file.
            let updated = get_updated_import_specifier_from_moved_source_files(
                program,
                source_file,
                import_literal,
                old_to_new,
                new_import_from_path,
                user_preferences,
            );
            let import_text = source_file.store().text(*import_literal);
            if !updated.is_empty() && updated != import_text {
                return updated;
            }
            // Fall back to a regular path update for unresolved module.
            if tspath::is_external_module_name_relative(&import_text) {
                return self.update_relative_path(
                    old_to_new,
                    &source_file.file_name(),
                    new_import_from_path,
                    &import_text,
                );
            }
            return String::new();
        };

        // Optimization: neither the importing or imported file changed.
        if !target.updated
            && !(importing_source_file_moved
                && tspath::is_external_module_name_relative(
                    &source_file.store().text(*import_literal),
                ))
        {
            return String::new();
        }

        get_updated_module_specifier(
            program,
            source_file,
            import_literal,
            new_import_from_path,
            &target.new_file_name,
            user_preferences,
        )
    }
}

pub fn get_updated_module_specifier(
    program: &compiler::Program,
    source_file: &ast::SourceFile,
    import_literal: &ast::StringLiteralLike,
    new_import_from_path: &str,
    new_file_name: &str,
    user_preferences: modulespecifiers::UserPreferences,
) -> String {
    let importing_source_file =
        ImportingSourceFileForSpecifierGeneration::new(source_file, new_import_from_path);
    let (module_specifiers, _) = modulespecifiers::get_module_specifiers_for_file_with_info(
        &importing_source_file,
        new_file_name,
        program.compiler_options(),
        program,
        user_preferences.clone(),
        modulespecifiers::ModuleSpecifierOptions {
            override_import_mode: program.get_mode_for_usage_location(source_file, import_literal),
        },
        false,
    );
    modulespecifiers::update_module_specifier(
        program.compiler_options(),
        program,
        &importing_source_file,
        new_import_from_path,
        &source_file.store().text(*import_literal),
        module_specifiers.first().map(String::as_str).unwrap_or(""),
        modulespecifiers::UpdateModuleSpecifierOptions {
            user_preferences,
            options: modulespecifiers::ModuleSpecifierOptions {
                override_import_mode: program
                    .get_mode_for_usage_location(source_file, import_literal),
            },
        },
    )
}

pub fn update_paths_property<'a>(
    config_file: &'a ast::SourceFile,
    config_dir: &str,
    property: &ast::Node,
    change_tracker: &mut change::Tracker<'a>,
    old_to_new: &PathUpdater,
    converters: &lsconv::Converters,
    use_case_sensitive_file_names: bool,
) -> bool {
    let Some(initializer) = config_file.store().initializer(*property) else {
        return false;
    };
    let mut elements = vec![initializer];
    if ast::is_array_literal_expression(config_file.store(), initializer) {
        elements = config_file
            .store()
            .elements(initializer)
            .map(|elements| elements.iter().collect())
            .unwrap_or_default();
    }

    let mut found_exact_match = false;
    for element in elements {
        found_exact_match = try_update_config_string(
            config_file,
            config_dir,
            &element,
            change_tracker,
            old_to_new,
            converters,
            use_case_sensitive_file_names,
        ) || found_exact_match;
    }
    found_exact_match
}

pub fn try_update_config_string<'a>(
    config_file: &'a ast::SourceFile,
    config_dir: &str,
    element: &ast::Node,
    change_tracker: &mut change::Tracker<'a>,
    old_to_new: &PathUpdater,
    converters: &lsconv::Converters,
    use_case_sensitive_file_names: bool,
) -> bool {
    if !ast::is_string_literal(config_file.store(), *element) {
        return false;
    }

    let element_file_name = tspath::normalize_path(&tspath::combine_paths(
        config_dir,
        &[config_file.store().text(*element).as_str()],
    ));
    let (updated, ok) = old_to_new(&element_file_name);
    if !ok {
        return false;
    }

    change_tracker.replace_range_with_text(
        config_file,
        lsproto::Range {
            start: converters.position_to_line_and_character(
                config_file,
                core::TextPos(
                    scanner::get_token_pos_of_node(element, config_file, false) as i32 + 1,
                ),
            ),
            end: converters.position_to_line_and_character(
                config_file,
                core::TextPos(config_file.store().loc(*element).end() - 1),
            ),
        },
        &relative_path_from_directory(config_dir, &updated, use_case_sensitive_file_names),
    );
    true
}

pub fn get_source_file_to_import(
    program: &compiler::Program,
    source_file: &ast::SourceFile,
    import_literal: &ast::StringLiteralLike,
    old_to_new: &PathUpdater,
) -> Option<ToImport> {
    if let Some(resolved) =
        program.get_resolved_module_from_module_specifier(source_file, import_literal)
    {
        if !resolved.resolved_file_name.is_empty() {
            let old_file_name = String::from(resolved.resolved_file_name.as_str());
            let (new_file_name, ok) = old_to_new(&old_file_name);
            if ok {
                return Some(ToImport {
                    new_file_name,
                    updated: true,
                });
            }
            return Some(ToImport {
                new_file_name: old_file_name,
                updated: false,
            });
        }
    }
    None
}

// As a fall back for unresolved modules, we'll check all files in the program to see if any of them would match
// the import specifier, and if so, we'll obtain the updated specifier for that file.
pub fn get_updated_import_specifier_from_moved_source_files(
    program: &compiler::Program,
    source_file: &ast::SourceFile,
    import_literal: &ast::StringLiteralLike,
    old_to_new: &PathUpdater,
    importing_source_file_name: &str,
    user_preferences: modulespecifiers::UserPreferences,
) -> String {
    for candidate in program.get_parsed_source_files_refs() {
        let (new_file_name, ok) = old_to_new(&candidate.file_name());
        if !ok {
            continue;
        }

        let old_specifier = get_updated_module_specifier(
            program,
            source_file,
            import_literal,
            importing_source_file_name,
            &candidate.file_name(),
            user_preferences.clone(),
        );
        if old_specifier != source_file.store().text(*import_literal) {
            continue;
        }

        return get_updated_module_specifier(
            program,
            source_file,
            import_literal,
            importing_source_file_name,
            &new_file_name,
            user_preferences,
        );
    }
    String::new()
}

pub fn create_string_text_range(
    source_file: &ast::SourceFile,
    node: &ast::Node,
) -> core::TextRange {
    core::new_text_range(
        scanner::get_token_pos_of_node(node, source_file, false) as i32 + 1,
        source_file.store().loc(*node).end() - 1,
    )
}

pub fn get_ts_config_object_literal_expression(
    ts_config_source_file: &ast::SourceFile,
) -> Option<ast::Node> {
    let statements = ts_config_source_file.statements_view();
    if let Some(statement) = statements.iter().next() {
        let expression = ts_config_source_file.store().expression(statement);
        if expression.as_ref().is_some_and(|expression| {
            ast::is_object_literal_expression(ts_config_source_file.store(), *expression)
        }) {
            return expression;
        }
    }
    None
}

pub fn for_each_object_property(
    store: &ast::AstStore,
    object_literal: ast::Node,
    mut cb: impl FnMut(&ast::Node, String),
) {
    let Some(properties) = store.properties(object_literal) else {
        return;
    };
    for property in properties.iter() {
        if !ast::is_property_assignment(store, property) {
            continue;
        }
        if let Some(name) = store.name(property) {
            let (name, ok) = ast::try_get_text_of_property_name(store, name);
            if ok {
                cb(&property, name);
            }
        }
    }
}

pub fn relative_path_from_directory(
    from_directory: &str,
    to: &str,
    use_case_sensitive_file_names: bool,
) -> String {
    tspath::get_relative_path_from_directory(
        from_directory,
        to,
        &tspath::ComparePathsOptions {
            use_case_sensitive_file_names,
            current_directory: String::new(),
        },
    )
}

pub fn relative_import_path_from_directory(
    from_directory: &str,
    to: &str,
    use_case_sensitive_file_names: bool,
) -> String {
    tspath::ensure_path_is_non_module_name(&relative_path_from_directory(
        from_directory,
        to,
        use_case_sensitive_file_names,
    ))
}

pub(crate) fn is_ambient_module_symbol(
    checker: &mut checker::Checker<'_, '_>,
    symbol: Option<ast::SymbolIdentity>,
) -> bool {
    let Some(symbol) = symbol else {
        return false;
    };
    checker
        .collect_symbol_declarations_public(symbol)
        .iter()
        .any(|declaration| {
            let store = modulespecifiers::CheckerShape::source_file_store(checker, *declaration)
                .expect("ambient module declaration should belong to a checker source file");
            ast::is_module_with_string_literal_name(store, *declaration)
        })
}
