use std::cmp;
use std::collections::BTreeMap;
use std::fmt;

use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_collections as collections;
use ts_compiler as compiler;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_locale as locale;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;
use ts_scanner as scanner;
use ts_stringutil as stringutil;
use ts_tspath as tspath;

use crate::autoimport::{Export, ExportSyntax, ModuleId, View};
use crate::change;
use crate::lsconv;
use crate::lsutil;

#[derive(Clone, Debug, Default)]
pub struct NewImportBinding {
    pub kind: Option<lsproto::ImportKind>,
    pub property_name: String,
    pub name: String,
    pub add_as_type_only: Option<lsproto::AddAsTypeOnly>,
}

#[derive(Clone, Debug, Default)]
pub struct Fix {
    pub auto_import_fix: Option<lsproto::AutoImportFix>,
    pub module_specifier_kind: Option<modulespecifiers::ResultKind>,
    pub is_re_export: bool,
    pub module_file_name: String,
    pub type_only_alias_declaration: Option<ast::Node>,
}

#[derive(Clone, Debug, Default)]
pub struct AddToExistingImportFix {
    pub import_clause_or_binding_pattern: Option<ast::Node>,
    // One of `defaultImport` or `namedImports` will be present
    pub default_import: Option<NewImportBinding>,
    pub named_import: Option<NewImportBinding>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExistingImport {
    pub node: Option<ast::Node>,
    pub module_specifier: String,
    pub index: usize,
}

impl Fix {
    pub fn kind(&self) -> Option<lsproto::AutoImportFixKind> {
        self.auto_import_fix.as_ref().map(|fix| fix.kind)
    }

    pub fn module_specifier(&self) -> &str {
        self.auto_import_fix
            .as_ref()
            .map(|fix| fix.module_specifier.as_str())
            .unwrap_or("")
    }

    pub fn import_kind(&self) -> Option<lsproto::ImportKind> {
        self.auto_import_fix.as_ref().map(|fix| fix.import_kind)
    }

    pub fn edits(
        &self,
        ctx: core::Context,
        file: &ast::SourceFile,
        compiler_options: &core::CompilerOptions,
        format_options: lsutil::FormatCodeSettings,
        converters: &lsconv::Converters,
        preferences: lsutil::UserPreferences,
    ) -> (Vec<lsproto::TextEdit>, String) {
        let Some(auto_import_fix) = self.auto_import_fix.as_ref() else {
            panic!("missing auto import fix");
        };
        let locale = locale::und();
        let mut tracker = change::new_tracker(ctx, compiler_options, format_options, converters);

        let description = match auto_import_fix.kind {
            lsproto::AutoImportFixKind::UseNamespace => {
                add_namespace_qualifier(self, &mut tracker, file, locale)
            }
            lsproto::AutoImportFixKind::AddToExisting => {
                if auto_import_fix.import_index < 0
                    || file.imports().len() <= auto_import_fix.import_index as usize
                {
                    panic!("import index out of range");
                }
                let Some(existing_fix) = get_add_to_existing_import_fix(file, self) else {
                    panic!("expected add to existing import fix");
                };
                add_to_existing_import(
                    &mut tracker,
                    file,
                    existing_fix
                        .import_clause_or_binding_pattern
                        .as_ref()
                        .expect("expected import clause or binding pattern"),
                    existing_fix.default_import.as_ref(),
                    &existing_fix
                        .named_import
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>(),
                    preferences,
                );
                diagnostics::localize(
                    locale,
                    Some(&*diagnostics::Update_import_from_0),
                    String::new(),
                    [auto_import_fix.module_specifier.clone()],
                )
            }
            lsproto::AutoImportFixKind::AddNew => {
                let quote_preference = lsutil::get_quote_preference(file, &preferences);
                let default_import = if auto_import_fix.import_kind == lsproto::ImportKind::Default
                {
                    Some(NewImportBinding {
                        name: auto_import_fix.name.clone(),
                        add_as_type_only: Some(auto_import_fix.add_as_type_only),
                        ..Default::default()
                    })
                } else {
                    None
                };
                let named_imports = if auto_import_fix.import_kind == lsproto::ImportKind::Named {
                    vec![NewImportBinding {
                        name: auto_import_fix.name.clone(),
                        add_as_type_only: Some(auto_import_fix.add_as_type_only),
                        ..Default::default()
                    }]
                } else {
                    Vec::new()
                };
                let namespace_like_import = if auto_import_fix.import_kind
                    == lsproto::ImportKind::Namespace
                    || auto_import_fix.import_kind == lsproto::ImportKind::CommonJS
                {
                    Some(NewImportBinding {
                        kind: Some(auto_import_fix.import_kind),
                        name: auto_import_fix.name.clone(),
                        ..Default::default()
                    })
                } else {
                    None
                };
                let import_text = make_new_import_text_from_bindings(
                    &auto_import_fix.module_specifier,
                    quote_preference,
                    auto_import_fix.use_require,
                    default_import.as_ref(),
                    &named_imports,
                    namespace_like_import.as_ref(),
                    compiler_options,
                    preferences.clone(),
                );
                insert_import_texts(
                    &mut tracker,
                    file,
                    import_text,
                    &auto_import_fix.module_specifier,
                    auto_import_fix.use_require,
                    true, /*blankLineBetween*/
                    preferences,
                );
                diagnostics::localize(
                    locale,
                    Some(&*diagnostics::Add_import_from_0),
                    String::new(),
                    [auto_import_fix.module_specifier.clone()],
                )
            }
            lsproto::AutoImportFixKind::PromoteTypeOnly => {
                let Some(type_only_alias_declaration) = self.type_only_alias_declaration.as_ref()
                else {
                    panic!("missing type-only alias declaration");
                };
                let Some(promoted_declaration) = promote_from_type_only(
                    &mut tracker,
                    type_only_alias_declaration,
                    compiler_options,
                    file,
                    preferences,
                ) else {
                    panic!("expected promoted type-only declaration");
                };
                if file.store().kind(promoted_declaration) == ast::Kind::ImportSpecifier {
                    let store = file.store();
                    let import_declaration = store
                        .parent(promoted_declaration)
                        .and_then(|parent| store.parent(parent))
                        .expect("import specifier should have import declaration parent");
                    let module_specifier = get_module_specifier_text(store, &import_declaration);
                    diagnostics::localize(
                        locale,
                        Some(&*diagnostics::Remove_type_from_import_of_0_from_1),
                        String::new(),
                        [auto_import_fix.name.clone(), module_specifier],
                    )
                } else {
                    let module_specifier =
                        get_module_specifier_text(file.store(), &promoted_declaration);
                    diagnostics::localize(
                        locale,
                        Some(&*diagnostics::Remove_type_from_import_declaration_from_0),
                        String::new(),
                        [module_specifier],
                    )
                }
            }
            _ => panic!("unsupported auto import fix kind"),
        };

        let mut changes = tracker.get_changes();
        (
            changes.remove(&file.file_name()).unwrap_or_default(),
            description,
        )
    }
}

pub fn add_namespace_qualifier<'a>(
    fix: &Fix,
    tracker: &mut change::Tracker<'a>,
    file: &'a ast::SourceFile,
    locale: locale::Locale,
) -> String {
    let Some(auto_import_fix) = fix.auto_import_fix.as_ref() else {
        panic!("namespace fix requires usage position and prefix");
    };
    let Some(usage_position) = auto_import_fix.usage_position else {
        panic!("namespace fix requires usage position and prefix");
    };
    if auto_import_fix.namespace_prefix.is_empty() {
        panic!("namespace fix requires usage position and prefix");
    }
    let qualified = format!(
        "{}.{}",
        auto_import_fix.namespace_prefix, auto_import_fix.name
    );
    let prefix = auto_import_fix.namespace_prefix.clone() + ".";
    tracker.insert_text(file, usage_position, &prefix);
    diagnostics::localize(
        locale,
        Some(&*diagnostics::Change_0_to_1),
        String::new(),
        [auto_import_fix.name.clone(), qualified],
    )
}

pub fn get_add_to_existing_import_fix(
    file: &ast::SourceFile,
    fix: &Fix,
) -> Option<AddToExistingImportFix> {
    let store = file.store();
    let Some(auto_import_fix) = fix.auto_import_fix.as_ref() else {
        panic!("expected add to existing import fix");
    };
    if auto_import_fix.kind != lsproto::AutoImportFixKind::AddToExisting {
        panic!("expected add to existing import fix");
    }
    if auto_import_fix.import_index < 0 {
        panic!("import index out of range");
    }
    let Some(module_specifier) = file.imports().get(auto_import_fix.import_index as usize) else {
        panic!("import index out of range");
    };
    let Some(import_node) = ast::try_get_import_from_module_specifier(store, module_specifier)
    else {
        panic!("expected import declaration");
    };
    let import_clause_or_binding_pattern = match store.kind(import_node) {
        ast::Kind::ImportDeclaration => {
            let Some(import_clause) = store.import_clause(import_node) else {
                panic!("expected import clause");
            };
            import_clause
        }
        ast::Kind::CallExpression => {
            let parent = store
                .parent(import_node)
                .expect("expected require call expression to have a parent");
            if !ast::is_variable_declaration_initialized_to_require(store, parent) {
                panic!("expected require call expression to be in variable declaration");
            }
            let Some(name) = store.name(parent) else {
                panic!("expected object binding pattern in variable declaration");
            };
            if store.kind(name) != ast::Kind::ObjectBindingPattern {
                panic!("expected object binding pattern in variable declaration");
            }
            name
        }
        _ => panic!("expected import declaration or require call expression"),
    };

    let default_import = if auto_import_fix.import_kind == lsproto::ImportKind::Default {
        Some(NewImportBinding {
            kind: Some(lsproto::ImportKind::Default),
            name: auto_import_fix.name.clone(),
            add_as_type_only: Some(auto_import_fix.add_as_type_only),
            ..Default::default()
        })
    } else {
        None
    };
    let named_import = if auto_import_fix.import_kind == lsproto::ImportKind::Named {
        Some(NewImportBinding {
            kind: Some(lsproto::ImportKind::Named),
            name: auto_import_fix.name.clone(),
            add_as_type_only: Some(auto_import_fix.add_as_type_only),
            ..Default::default()
        })
    } else {
        None
    };

    Some(AddToExistingImportFix {
        import_clause_or_binding_pattern: Some(import_clause_or_binding_pattern),
        default_import,
        named_import,
    })
}

pub fn add_to_existing_import<'a>(
    tracker: &mut change::Tracker<'a>,
    file: &'a ast::SourceFile,
    import_clause_or_binding_pattern: &ast::Node,
    default_import: Option<&NewImportBinding>,
    named_imports: &[NewImportBinding],
    preferences: lsutil::UserPreferences,
) {
    let store = file.store();
    match store.kind(*import_clause_or_binding_pattern) {
        ast::Kind::ObjectBindingPattern => {
            if let Some(default_import) = default_import {
                add_element_to_binding_pattern(
                    tracker,
                    file,
                    *import_clause_or_binding_pattern,
                    &default_import.name,
                    "default",
                );
            }
            for named_import in named_imports {
                add_element_to_binding_pattern(
                    tracker,
                    file,
                    *import_clause_or_binding_pattern,
                    &named_import.name,
                    "",
                );
            }
        }
        ast::Kind::ImportClause => {
            // promoteFromTypeOnly = true if we need to promote the entire original clause from type only
            let promote_from_type_only = store
                .is_type_only(*import_clause_or_binding_pattern)
                .unwrap_or(false)
                && named_imports
                    .iter()
                    .map(Some)
                    .chain(std::iter::once(default_import))
                    .any(|import| {
                        import.and_then(|import| import.add_as_type_only)
                            == Some(lsproto::AddAsTypeOnly::NotAllowed)
                    });

            let existing_specifiers = if let Some(named_bindings) =
                store.named_bindings(*import_clause_or_binding_pattern)
            {
                if store.kind(named_bindings) == ast::Kind::NamedImports {
                    store
                        .elements(named_bindings)
                        .map(|elements| elements.iter().collect())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            if let Some(default_import) = default_import {
                if store.name(*import_clause_or_binding_pattern).is_some() {
                    panic!("Cannot add a default import to an import clause that already has one");
                }
                insert_text_at_pos(
                    tracker,
                    file,
                    store.loc(*import_clause_or_binding_pattern).pos(),
                    &(default_import.name.clone() + ", "),
                );
            }

            if !named_imports.is_empty() {
                let named_bindings = store.named_bindings(*import_clause_or_binding_pattern);
                let import_decl = store
                    .parent(*import_clause_or_binding_pattern)
                    .unwrap_or(*import_clause_or_binding_pattern);
                let sorting = lsutil::get_named_import_specifier_sorting_with_detection(
                    store,
                    &import_decl,
                    Some(file),
                    preferences.clone(),
                );
                let compare_preferences = lsutil::UserPreferences {
                    organize_imports_type_order: sorting.type_order,
                    ..lsutil::UserPreferences::default()
                };
                let string_comparer = sorting.string_comparer;
                let mut new_specifiers = named_imports
                    .iter()
                    .map(|named_import| {
                        let is_type_only = (!store
                            .is_type_only(*import_clause_or_binding_pattern)
                            .unwrap_or(false)
                            || promote_from_type_only)
                            && should_use_type_only(
                                named_import
                                    .add_as_type_only
                                    .unwrap_or(lsproto::AddAsTypeOnly::Allowed),
                                preferences.clone(),
                            );
                        NewImportSpecifierForInsert {
                            name: named_import.name.clone(),
                            property_name: named_import.property_name.clone(),
                            is_type_only,
                        }
                    })
                    .collect::<Vec<_>>();
                new_specifiers.sort_by(|a, b| {
                    compare_new_import_specifiers_for_insert(
                        a,
                        b,
                        compare_preferences.clone(),
                        string_comparer.as_ref(),
                    )
                    .cmp(&0)
                });

                if !existing_specifiers.is_empty() && sorting.is_sorted != core::Tristate::False {
                    // The sorting preference computed earlier may or may not have validated that these particular
                    // import specifiers are sorted. If they aren't, `getImportSpecifierInsertionIndex` will return
                    // nonsense. So if there are existing specifiers, even if we know the sorting preference, we
                    // need to ensure that the existing specifiers are sorted according to the preference in order
                    // to do a sorted insertion.

                    // If we're promoting the clause from type-only, we need to transform the existing imports
                    // before attempting to insert the new named imports (for comparison purposes only)
                    let named_bindings = named_bindings.expect("expected named imports");
                    let mut specifiers_by_index = BTreeMap::new();
                    for spec in new_specifiers {
                        let insertion_index = get_import_specifier_insertion_index_for_binding(
                            store,
                            &existing_specifiers,
                            &spec,
                            compare_preferences.clone(),
                            string_comparer.as_ref(),
                            promote_from_type_only,
                        );
                        specifiers_by_index
                            .entry(insertion_index)
                            .or_insert_with(Vec::new)
                            .push(import_specifier_text_for_insert(&spec));
                    }
                    for (insertion_index, specifiers) in specifiers_by_index {
                        insert_import_specifiers_at_index(
                            tracker,
                            file,
                            &existing_specifiers,
                            named_bindings,
                            insertion_index,
                            &specifiers,
                        );
                    }
                } else if let Some(last_specifier) = existing_specifiers.last() {
                    let new_specifiers = new_specifiers
                        .iter()
                        .map(import_specifier_text_for_insert)
                        .collect::<Vec<_>>();
                    insert_import_specifiers_at_index(
                        tracker,
                        file,
                        &existing_specifiers,
                        store
                            .parent(*last_specifier)
                            .expect("expected named imports"),
                        existing_specifiers.len(),
                        &new_specifiers,
                    );
                } else if let Some(named_bindings) = named_bindings {
                    if let Some(open_brace_pos) = find_byte_in_node(file, &named_bindings, b'{') {
                        let new_specifiers = new_specifiers
                            .iter()
                            .map(import_specifier_text_for_insert)
                            .collect::<Vec<_>>();
                        insert_text_at_pos(
                            tracker,
                            file,
                            open_brace_pos + 1,
                            &(" ".to_string() + &new_specifiers.join(", ") + " "),
                        );
                    }
                } else if let Some(name) = store.name(*import_clause_or_binding_pattern) {
                    let new_specifiers = new_specifiers
                        .iter()
                        .map(import_specifier_text_for_insert)
                        .collect::<Vec<_>>();
                    insert_text_at_pos(
                        tracker,
                        file,
                        store.loc(name).end(),
                        &(", { ".to_string() + &new_specifiers.join(", ") + " }"),
                    );
                } else {
                    panic!("Import clause must have either named imports or a default import");
                }
            }

            if promote_from_type_only {
                // Delete the 'type' keyword from the import clause
                delete_type_keyword(
                    tracker,
                    file,
                    store.loc(*import_clause_or_binding_pattern).pos(),
                );

                // Add 'type' modifier to existing specifiers (not newly added ones)
                // We preserve the type-onlyness of existing specifiers regardless of whether
                // it would make a difference in emit (user preference).
                for specifier in existing_specifiers {
                    if !store.is_type_only(specifier).unwrap_or(false) {
                        insert_text_at_pos(tracker, file, store.loc(specifier).pos(), "type ");
                    }
                }
            }
        }
        _ => panic!(
            "Unsupported clause kind: {} for addToExistingImport",
            store.kind(*import_clause_or_binding_pattern)
        ),
    }
}

struct NewImportSpecifierForInsert {
    name: String,
    property_name: String,
    is_type_only: bool,
}

fn compare_new_import_specifiers_for_insert(
    a: &NewImportSpecifierForInsert,
    b: &NewImportSpecifierForInsert,
    preferences: lsutil::UserPreferences,
    string_comparer: &dyn Fn(&str, &str) -> i32,
) -> i32 {
    compare_import_specifier_parts(
        a.is_type_only,
        &a.name,
        b.is_type_only,
        &b.name,
        preferences,
        string_comparer,
    )
}

fn get_import_specifier_insertion_index_for_binding(
    store: &ast::AstStore,
    sorted_imports: &[ast::Node],
    new_import: &NewImportSpecifierForInsert,
    preferences: lsutil::UserPreferences,
    string_comparer: &dyn Fn(&str, &str) -> i32,
    promote_from_type_only: bool,
) -> usize {
    let (index, found) = core::binary_search_unique_func(sorted_imports, |_mid, value| {
        let existing_is_type_only =
            promote_from_type_only || store.is_type_only(*value).unwrap_or(false);
        let existing_name = store.text(store.name(*value).unwrap());
        match compare_import_specifier_parts(
            existing_is_type_only,
            &existing_name,
            new_import.is_type_only,
            &new_import.name,
            preferences.clone(),
            string_comparer,
        ) {
            x if x < 0 => std::cmp::Ordering::Less,
            x if x > 0 => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        }
    });
    if found { index } else { index }
}

fn compare_import_specifier_parts(
    left_is_type_only: bool,
    left_name: &str,
    right_is_type_only: bool,
    right_name: &str,
    preferences: lsutil::UserPreferences,
    string_comparer: &dyn Fn(&str, &str) -> i32,
) -> i32 {
    match preferences.organize_imports_type_order {
        lsutil::OrganizeImportsTypeOrder::First => {
            let cmp = core::compare_booleans(right_is_type_only, left_is_type_only);
            if cmp != 0 {
                return cmp;
            }
            string_comparer(left_name, right_name)
        }
        lsutil::OrganizeImportsTypeOrder::Inline => string_comparer(left_name, right_name),
        lsutil::OrganizeImportsTypeOrder::Last | lsutil::OrganizeImportsTypeOrder::Auto => {
            let cmp = core::compare_booleans(left_is_type_only, right_is_type_only);
            if cmp != 0 {
                return cmp;
            }
            string_comparer(left_name, right_name)
        }
    }
}

fn import_specifier_text_for_insert(specifier: &NewImportSpecifierForInsert) -> String {
    let mut text = String::new();
    if specifier.is_type_only {
        text.push_str("type ");
    }
    if !specifier.property_name.is_empty() {
        text.push_str(&specifier.property_name);
        text.push_str(" as ");
    }
    text.push_str(&specifier.name);
    text
}

fn insert_import_specifiers_at_index<'a>(
    tracker: &mut change::Tracker<'a>,
    file: &'a ast::SourceFile,
    existing_specifiers: &[ast::Node],
    _named_imports: ast::Node,
    index: usize,
    specifiers: &[String],
) {
    if specifiers.is_empty() {
        return;
    }

    let text = specifiers.join(", ");
    let store = file.store();
    if index < existing_specifiers.len() {
        let target = existing_specifiers[index];
        let options = scanner::SkipTriviaOptions {
            stop_after_line_break: false,
            stop_at_comments: true,
        };
        let start_pos = scanner::skip_trivia_ex(
            file.text(),
            store.loc(target).pos() as usize,
            Some(&options),
        ) as i32;
        let suffix = if index > 0 {
            let previous = existing_specifiers[index - 1];
            separator_suffix_after_specifier(file, previous, start_pos)
                .unwrap_or_else(|| ", ".to_string())
        } else {
            ", ".to_string()
        };
        insert_text_at_pos(tracker, file, start_pos, &(text + &suffix));
    } else if let Some(last_specifier) = existing_specifiers.last() {
        insert_text_at_pos(
            tracker,
            file,
            store.loc(*last_specifier).end(),
            &(", ".to_string() + &text),
        );
    }
}

fn separator_suffix_after_specifier(
    file: &ast::SourceFile,
    specifier: ast::Node,
    next_start: i32,
) -> Option<String> {
    let store = file.store();
    let token = astnav::get_token_at_position_info(file, store.loc(specifier).end())?;
    if !is_separator_token_kind(store, specifier, token.kind) {
        return None;
    }
    Some(
        scanner::token_to_string(token.kind)
            + &file.text()[token.loc.end() as usize..next_start as usize],
    )
}

fn is_separator_token_kind(store: &ast::AstStore, node: ast::Node, candidate: ast::Kind) -> bool {
    store.parent(node).is_some()
        && (candidate == ast::Kind::CommaToken
            || candidate == ast::Kind::SemicolonToken
                && store
                    .parent(node)
                    .is_some_and(|parent| store.kind(parent) == ast::Kind::ObjectLiteralExpression))
}

pub fn add_element_to_binding_pattern<'a>(
    tracker: &mut change::Tracker<'a>,
    file: &'a ast::SourceFile,
    binding_pattern_node: ast::Node,
    name: &str,
    property_name: &str,
) {
    let store = file.store();
    let text = if property_name.is_empty() {
        name.to_string()
    } else {
        format!("{property_name}: {name}")
    };
    if let Some(last_element) = store
        .elements(binding_pattern_node)
        .and_then(|elements| elements.last())
    {
        insert_text_at_pos(
            tracker,
            file,
            store.loc(last_element).end(),
            &(", ".to_string() + &text),
        );
    } else if let Some(open_brace_pos) = find_byte_in_node(file, &binding_pattern_node, b'{') {
        insert_text_at_pos(
            tracker,
            file,
            open_brace_pos + 1,
            &(" ".to_string() + &text + " "),
        );
    }
}

fn insert_text_at_pos<'a>(
    tracker: &mut change::Tracker<'a>,
    file: &'a ast::SourceFile,
    pos: i32,
    text: &str,
) {
    let position = tracker.converters.position_to_line_and_character(file, pos);
    tracker.insert_text(file, position, text);
}

pub(crate) fn insert_import_texts<'a>(
    tracker: &mut change::Tracker<'a>,
    file: &'a ast::SourceFile,
    mut imports: Vec<String>,
    module_specifier: &str,
    use_require: bool,
    blank_line_between: bool,
    preferences: lsutil::UserPreferences,
) {
    let store = file.store();
    let existing_import_statements = existing_import_or_require_statements(file, use_require);
    imports.sort();
    let import_text = imports.join(&tracker.new_line);

    if existing_import_statements.is_empty() {
        let pos = tracker.get_insertion_position_at_source_file_top(file);
        let mut text = String::new();
        if pos != 0 {
            text.push_str(&tracker.new_line);
        }
        text.push_str(&import_text);
        let needs_suffix = file
            .text()
            .as_bytes()
            .get(pos as usize)
            .is_none_or(|ch| !stringutil::is_line_break(*ch as char));
        if needs_suffix {
            text.push_str(&tracker.new_line);
        }
        if blank_line_between {
            text.push_str(&tracker.new_line);
        }
        insert_text_at_pos(tracker, file, pos, &text);
        return;
    }

    let (comparer, is_sorted) = lsutil::get_organize_imports_string_comparer_with_detection(
        store,
        &existing_import_statements,
        preferences,
    );
    if is_sorted {
        let insertion_index = existing_import_statements
            .iter()
            .position(|existing| {
                let existing_module = lsutil::get_external_module_name(
                    store,
                    lsutil::get_module_specifier_expression(store, existing).as_ref(),
                );
                compare_module_specifier_names(&existing_module, module_specifier, &*comparer) > 0
            })
            .unwrap_or(existing_import_statements.len());
        if insertion_index == 0 {
            let first_import = existing_import_statements[0];
            let first_statement = store
                .statements(file.as_node())
                .and_then(|statements| statements.first());
            let leading_trivia_option = if Some(first_import) == first_statement {
                change::LEADING_TRIVIA_OPTION_EXCLUDE
            } else {
                change::LEADING_TRIVIA_OPTION_NONE
            };
            let pos = tracker.get_adjusted_start_position(
                file,
                first_import,
                leading_trivia_option,
                false,
            );
            insert_text_at_pos(
                tracker,
                file,
                pos,
                &(import_text + &tracker.new_line.clone()),
            );
            return;
        }

        let previous_import = existing_import_statements[insertion_index - 1];
        let pos = tracker.get_adjusted_end_position(
            file,
            previous_import,
            change::TRAILING_TRIVIA_OPTION_NONE,
        );
        insert_text_at_pos(
            tracker,
            file,
            pos,
            &(import_text + &tracker.new_line.clone()),
        );
        return;
    }

    let previous_import = *existing_import_statements
        .last()
        .expect("existing import statements should be non-empty");
    let pos = tracker.get_adjusted_end_position(
        file,
        previous_import,
        change::TRAILING_TRIVIA_OPTION_NONE,
    );
    insert_text_at_pos(
        tracker,
        file,
        pos,
        &(import_text + &tracker.new_line.clone()),
    );
}

fn existing_import_or_require_statements(
    file: &ast::SourceFile,
    use_require: bool,
) -> Vec<ast::Statement> {
    let store = file.store();
    let Some(statements) = store.statements(file.as_node()) else {
        return Vec::new();
    };
    statements
        .iter()
        .filter(|statement| {
            if use_require {
                is_require_variable_statement(store, *statement)
            } else {
                ast::is_any_import_syntax(store, *statement)
            }
        })
        .collect()
}

fn compare_module_specifier_names(
    left: &str,
    right: &str,
    comparer: &dyn Fn(&str, &str) -> i32,
) -> i32 {
    let cmp = core::compare_booleans(left.is_empty(), right.is_empty());
    if cmp != 0 {
        return cmp;
    }
    let cmp = core::compare_booleans(
        tspath::is_external_module_name_relative(left),
        tspath::is_external_module_name_relative(right),
    );
    if cmp != 0 {
        return cmp;
    }
    comparer(left, right)
}

pub(crate) fn has_existing_imports_or_requires(file: &ast::SourceFile, use_require: bool) -> bool {
    !existing_import_or_require_statements(file, use_require).is_empty()
}

fn is_require_variable_statement(store: &ast::AstStore, statement: ast::Node) -> bool {
    if store.kind(statement) != ast::Kind::VariableStatement {
        return false;
    }
    let Some(declaration_list) = store.declaration_list(statement) else {
        return false;
    };
    let Some(declarations) = store.declarations(declaration_list) else {
        return false;
    };
    declarations
        .iter()
        .any(|declaration| ast::is_variable_declaration_initialized_to_require(store, declaration))
}

fn find_byte_in_node(file: &ast::SourceFile, node: &ast::Node, byte: u8) -> Option<i32> {
    let text = file.text().as_bytes();
    let loc = file.store().loc(*node);
    let start = loc.pos().max(0) as usize;
    let end = loc.end().max(loc.pos()).min(file.text().len() as i32) as usize;
    text.get(start..end)?
        .iter()
        .position(|candidate| *candidate == byte)
        .map(|offset| (start + offset) as i32)
}

pub fn make_new_import_text_from_bindings(
    module_specifier: &str,
    quote_preference: lsutil::QuotePreference,
    use_require: bool,
    default_import: Option<&NewImportBinding>,
    named_imports: &[NewImportBinding],
    namespace_like_import: Option<&NewImportBinding>,
    compiler_options: &core::CompilerOptions,
    preferences: lsutil::UserPreferences,
) -> Vec<String> {
    let quote_char = if quote_preference == lsutil::QuotePreference::Single {
        "'"
    } else {
        "\""
    };
    let module_specifier = format!("{quote_char}{module_specifier}{quote_char}");
    let mut statements = Vec::new();

    if use_require {
        // const { default: foo, bar, etc } = require('./mod');
        if default_import.is_some() || !named_imports.is_empty() {
            let mut binding_elements = Vec::new();
            for named_import in named_imports {
                if named_import.property_name.is_empty() {
                    binding_elements.push(named_import.name.clone());
                } else {
                    binding_elements.push(format!(
                        "{}: {}",
                        named_import.property_name, named_import.name
                    ));
                }
            }
            if let Some(default_import) = default_import {
                binding_elements.insert(0, format!("default: {}", default_import.name));
            }
            statements.push(format!(
                "const {{ {} }} = require({module_specifier});",
                binding_elements.join(", ")
            ));
        }

        // const foo = require('./mod');
        if let Some(namespace_like_import) = namespace_like_import {
            statements.push(format!(
                "const {} = require({module_specifier});",
                namespace_like_import.name
            ));
        }
    } else {
        if default_import.is_some() || !named_imports.is_empty() {
            // `verbatimModuleSyntax` should prefer top-level `import type` -
            // even though it's not an error, it would add unnecessary runtime emit.
            let default_needs_type_only = default_import
                .and_then(|default_import| default_import.add_as_type_only)
                .map(needs_type_only)
                .unwrap_or(true);
            let top_level_type_only = (default_needs_type_only
                && named_imports.iter().all(|named_import| {
                    named_import.add_as_type_only.is_some_and(needs_type_only)
                }))
                || (compiler_options.verbatim_module_syntax.is_true()
                    || preferences.prefer_type_only_auto_imports.is_true())
                    && default_import
                        .and_then(|default_import| default_import.add_as_type_only)
                        .unwrap_or(lsproto::AddAsTypeOnly::Allowed)
                        != lsproto::AddAsTypeOnly::NotAllowed
                    && !named_imports.iter().any(|named_import| {
                        named_import.add_as_type_only == Some(lsproto::AddAsTypeOnly::NotAllowed)
                    });

            let default_text = default_import.map(|default_import| default_import.name.clone());
            let named_text = if named_imports.is_empty() {
                String::new()
            } else {
                let specifiers = named_imports
                    .iter()
                    .map(|named_import| {
                        let mut text = String::new();
                        if !top_level_type_only
                            && should_use_type_only(
                                named_import
                                    .add_as_type_only
                                    .unwrap_or(lsproto::AddAsTypeOnly::Allowed),
                                preferences.clone(),
                            )
                        {
                            text.push_str("type ");
                        }
                        if !named_import.property_name.is_empty() {
                            text.push_str(&named_import.property_name);
                            text.push_str(" as ");
                        }
                        text.push_str(&named_import.name);
                        text
                    })
                    .collect::<Vec<_>>();
                format!("{{ {} }}", specifiers.join(", "))
            };

            let phase = if top_level_type_only { "type " } else { "" };
            let clause = match (default_text, named_text.is_empty()) {
                (Some(default_text), true) => default_text,
                (Some(default_text), false) => format!("{default_text}, {named_text}"),
                (None, false) => named_text,
                (None, true) => String::new(),
            };
            statements.push(format!("import {phase}{clause} from {module_specifier};"));
        }

        if let Some(namespace_like_import) = namespace_like_import {
            let add_as_type_only = namespace_like_import
                .add_as_type_only
                .unwrap_or(lsproto::AddAsTypeOnly::Allowed);
            let type_prefix = if should_use_type_only(add_as_type_only, preferences) {
                "type "
            } else {
                ""
            };
            if namespace_like_import.kind == Some(lsproto::ImportKind::CommonJS) {
                statements.push(format!(
                    "import {type_prefix}{} = require({module_specifier});",
                    namespace_like_import.name
                ));
            } else {
                statements.push(format!(
                    "import {type_prefix}* as {} from {module_specifier};",
                    namespace_like_import.name
                ));
            }
        }
    }

    if statements.is_empty() {
        panic!("No statements to insert for new imports");
    }
    statements
}

impl View<'_> {
    pub fn get_fixes(
        &self,
        checker: &mut checker::Checker<'_, '_>,
        export: &Export,
        for_jsx: bool,
        is_valid_type_only_use_site: bool,
        usage_position: Option<&lsproto::Position>,
    ) -> Vec<Fix> {
        let mut fixes = Vec::new();
        if let Some(namespace_fix) =
            self.try_use_existing_namespace_import(checker, export, usage_position)
        {
            fixes.push(namespace_fix);
        }

        if let Some(fix) =
            self.try_add_to_existing_import(checker, export, is_valid_type_only_use_site)
        {
            fixes.push(fix);
            return fixes;
        }

        self.add_new_import_fixes(
            fixes,
            export,
            for_jsx,
            is_valid_type_only_use_site,
            usage_position,
        )
    }

    fn add_new_import_fixes(
        &self,
        mut fixes: Vec<Fix>,
        export: &Export,
        for_jsx: bool,
        is_valid_type_only_use_site: bool,
        usage_position: Option<&lsproto::Position>,
    ) -> Vec<Fix> {
        let (module_specifier, module_specifier_kind) =
            self.get_module_specifier(export, self.preferences.module_specifier_preferences());
        if module_specifier.is_empty() {
            return fixes;
        }

        let Some(importing_file) = self.importing_file.as_ref() else {
            return fixes;
        };
        let Some(program) = self.program else {
            return fixes;
        };

        let is_js = tspath::has_js_file_extension(&importing_file.file_name());
        let imported_symbol_has_value_meaning =
            export.flags & ast::SYMBOL_FLAGS_VALUE != 0 || export.is_unresolved_alias();
        if !imported_symbol_has_value_meaning && is_js && usage_position.is_some() {
            return fixes;
        }

        let import_kind = get_import_kind(importing_file, export, program);
        let add_as_type_only = get_add_as_type_only(
            is_valid_type_only_use_site,
            export,
            program.compiler_options(),
        );

        let mut name = export.name().to_string();
        let starts_with_upper = name.chars().next().is_some_and(|ch| ch.is_uppercase());
        if for_jsx && !starts_with_upper {
            if export.is_renameable() {
                let mut chars = name.chars();
                if let Some(first) = chars.next() {
                    name = first.to_uppercase().chain(chars).collect();
                }
            } else {
                return fixes;
            }
        }

        fixes.push(Fix {
            auto_import_fix: Some(lsproto::AutoImportFix {
                kind: lsproto::AutoImportFixKind::AddNew,
                import_kind,
                module_specifier,
                name,
                use_require: self.should_use_require(),
                add_as_type_only,
                ..Default::default()
            }),
            module_specifier_kind: Some(module_specifier_kind),
            is_re_export: export.target.module_id != export.export_id.module_id,
            module_file_name: export.module_file_name.clone(),
            ..Default::default()
        });
        fixes
    }

    pub fn try_use_existing_namespace_import(
        &self,
        checker: &mut checker::Checker<'_, '_>,
        export: &Export,
        usage_position: Option<&lsproto::Position>,
    ) -> Option<Fix> {
        if usage_position.is_none() {
            return None;
        }

        let Some(importing_file) = self.importing_file.as_ref() else {
            return None;
        };
        let Some(program) = self.program else {
            return None;
        };
        if get_import_kind(importing_file, export, program) != lsproto::ImportKind::Named {
            return None;
        }

        let existing_imports = self.get_existing_imports(checker);
        let Some(matching_declarations) = existing_imports.get(&export.export_id.module_id) else {
            return None;
        };
        for existing_import in matching_declarations {
            let Some(node) = existing_import.node.as_ref() else {
                continue;
            };
            let namespace_prefix = get_namespace_like_import_text(importing_file.store(), node);
            if namespace_prefix.is_empty() || existing_import.module_specifier.is_empty() {
                continue;
            }
            return Some(Fix {
                auto_import_fix: Some(lsproto::AutoImportFix {
                    kind: lsproto::AutoImportFixKind::UseNamespace,
                    name: export.name().to_string(),
                    module_specifier: existing_import.module_specifier.clone(),
                    import_kind: lsproto::ImportKind::Namespace,
                    add_as_type_only: lsproto::AddAsTypeOnly::Allowed,
                    import_index: existing_import.index as i32,
                    usage_position: usage_position.cloned(),
                    namespace_prefix,
                    ..Default::default()
                }),
                ..Default::default()
            });
        }

        None
    }

    pub fn try_add_to_existing_import(
        &self,
        checker: &mut checker::Checker<'_, '_>,
        export: &Export,
        is_valid_type_only_use_site: bool,
    ) -> Option<Fix> {
        let Some(importing_file) = self.importing_file.as_ref() else {
            return None;
        };
        let Some(program) = self.program else {
            return None;
        };
        let existing_imports = self.get_existing_imports(checker);
        let Some(matching_declarations) = existing_imports.get(&export.export_id.module_id) else {
            return None;
        };
        if matching_declarations.is_empty() {
            return None;
        }

        if ast::is_source_file_js(importing_file) && export.flags & ast::SYMBOL_FLAGS_VALUE == 0 {
            return None;
        }

        let import_kind = get_import_kind(importing_file, export, program);
        if import_kind == lsproto::ImportKind::CommonJS
            || import_kind == lsproto::ImportKind::Namespace
        {
            return None;
        }

        let add_as_type_only = get_add_as_type_only(
            is_valid_type_only_use_site,
            export,
            program.compiler_options(),
        );

        let mut best = None;
        for existing_import in matching_declarations {
            let Some(node) = existing_import.node.as_ref() else {
                continue;
            };
            if importing_file.store().kind(*node) == ast::Kind::ImportEqualsDeclaration {
                continue;
            }

            if importing_file.store().kind(*node) == ast::Kind::VariableDeclaration {
                if (import_kind == lsproto::ImportKind::Named
                    || import_kind == lsproto::ImportKind::Default)
                    && importing_file.store().name(*node).is_some_and(|name| {
                        importing_file.store().kind(name) == ast::Kind::ObjectBindingPattern
                    })
                {
                    let fix = Fix {
                        auto_import_fix: Some(lsproto::AutoImportFix {
                            kind: lsproto::AutoImportFixKind::AddToExisting,
                            name: export.name().to_string(),
                            import_kind,
                            import_index: existing_import.index as i32,
                            module_specifier: existing_import.module_specifier.clone(),
                            add_as_type_only,
                            ..Default::default()
                        }),
                        ..Default::default()
                    };
                    if add_as_type_only == lsproto::AddAsTypeOnly::NotAllowed {
                        return Some(fix);
                    }
                    if best.is_none() {
                        best = Some(fix);
                    }
                }
                continue;
            }

            let store = importing_file.store();
            let Some(import_clause_node) = store.import_clause(*node) else {
                continue;
            };
            if !store
                .module_specifier(*node)
                .is_some_and(|module_specifier| {
                    ast::is_string_literal_like(store, module_specifier)
                })
            {
                continue;
            }
            let named_bindings = store.named_bindings(import_clause_node);
            if store.is_type_only(import_clause_node).unwrap_or(false)
                && !(import_kind == lsproto::ImportKind::Named && named_bindings.is_some())
            {
                continue;
            }

            if import_kind == lsproto::ImportKind::Default
                && (store.name(import_clause_node).is_some()
                    || add_as_type_only == lsproto::AddAsTypeOnly::Required
                        && named_bindings.is_some())
            {
                continue;
            }

            if import_kind == lsproto::ImportKind::Named
                && named_bindings.is_some_and(|named_bindings| {
                    store.kind(named_bindings) == ast::Kind::NamespaceImport
                })
            {
                continue;
            }

            let fix = Fix {
                auto_import_fix: Some(lsproto::AutoImportFix {
                    kind: lsproto::AutoImportFixKind::AddToExisting,
                    name: export.name().to_string(),
                    import_kind,
                    import_index: existing_import.index as i32,
                    module_specifier: existing_import.module_specifier.clone(),
                    add_as_type_only,
                    ..Default::default()
                }),
                ..Default::default()
            };

            let is_type_only = importing_file
                .store()
                .is_type_only(import_clause_node)
                .unwrap_or(false);
            if (add_as_type_only != lsproto::AddAsTypeOnly::NotAllowed && is_type_only)
                || (add_as_type_only == lsproto::AddAsTypeOnly::NotAllowed && !is_type_only)
            {
                return Some(fix);
            }
            if best.is_none() {
                best = Some(fix);
            }
        }

        best
    }

    pub fn get_existing_imports(
        &self,
        checker: &mut checker::Checker<'_, '_>,
    ) -> collections::MultiMap<ModuleId, ExistingImport> {
        if let Some(existing_imports) = self.existing_imports.as_ref() {
            return existing_imports.clone();
        }

        let Some(importing_file) = self.importing_file.as_ref() else {
            return collections::MultiMap::default();
        };

        let mut result = collections::new_multi_map_with_size_hint(importing_file.imports().len());
        let store = importing_file.store();

        for (index, module_specifier) in importing_file.imports().iter().enumerate() {
            let Some(node) = ast::try_get_import_from_module_specifier(store, module_specifier)
            else {
                panic!(
                    "error: did not expect node kind {}",
                    store.kind(*module_specifier)
                );
            };
            let parent = store
                .parent(node)
                .expect("import node should have a parent");
            if ast::is_variable_declaration_initialized_to_require(store, parent) {
                if let Some(module_symbol) =
                    checker.resolve_external_module_name_public(*module_specifier)
                {
                    let (module_id, _, ok) =
                        crate::autoimport::util::try_get_module_id_and_file_name_of_module_symbol(
                            checker,
                            module_symbol,
                        );
                    if ok {
                        result.add(
                            module_id,
                            ExistingImport {
                                node: Some(parent),
                                module_specifier: store.text(*module_specifier),
                                index,
                            },
                        );
                    }
                }
            } else if matches!(
                store.kind(node),
                ast::Kind::ImportDeclaration | ast::Kind::ImportEqualsDeclaration
            ) {
                if let Some(module_symbol) =
                    checker.get_symbol_at_location_public(*module_specifier)
                {
                    let (module_id, _, ok) =
                        crate::autoimport::util::try_get_module_id_and_file_name_of_module_symbol(
                            checker,
                            module_symbol,
                        );
                    if ok {
                        result.add(
                            module_id,
                            ExistingImport {
                                node: Some(node),
                                module_specifier: store.text(*module_specifier),
                                index,
                            },
                        );
                    }
                }
            }
        }

        result
    }

    pub fn should_use_require(&self) -> bool {
        if let Some(should_use_require) = self.should_use_require_for_fixes {
            return should_use_require;
        }
        self.compute_should_use_require()
    }

    pub fn compute_should_use_require(&self) -> bool {
        let Some(importing_file) = self.importing_file.as_ref() else {
            return true;
        };
        let Some(program) = self.program else {
            return true;
        };

        // 1. TypeScript files don't use require variable declarations
        if !tspath::has_js_file_extension(&importing_file.file_name()) {
            return false;
        }

        // 2. If the current source file is unambiguously CJS or ESM, go with that
        match detect_syntax(importing_file, program.compiler_options()) {
            FileSyntaxKind::Cjs => return true,
            FileSyntaxKind::Esm => return false,
            FileSyntaxKind::Ambiguous => {}
        }

        // 3. Use the implied node format to determine CJS vs ESM
        match program.get_implied_node_format_for_emit_for_auto_imports(importing_file) {
            core::ModuleKind::CommonJS => return true,
            core::ModuleKind::ESNext => return false,
            _ => {}
        }

        // 4. If there's a tsconfig/jsconfig, use its module setting
        if !program.compiler_options().config_file_path.is_empty() {
            return program.compiler_options().get_emit_module_kind() < core::ModuleKind::ES2015;
        }

        // 5. Match the first other JS file in the program that's unambiguously CJS or ESM
        for other_file in program.source_files_for_auto_imports() {
            if other_file.file_name() == importing_file.file_name()
                || !ast::is_source_file_js(&other_file)
                || program.is_source_file_from_external_library_for_auto_imports(&other_file)
            {
                continue;
            }
            match detect_syntax(&other_file, program.compiler_options()) {
                FileSyntaxKind::Cjs => return true,
                FileSyntaxKind::Esm => return false,
                FileSyntaxKind::Ambiguous => {}
            }
        }

        // 6. Literally nothing to go on
        true
    }

    pub fn compare_fixes_for_sorting(&self, a: &Fix, b: &Fix) -> i32 {
        let ranking = self.compare_fixes_for_ranking(a, b);
        if ranking != 0 {
            return ranking;
        }
        self.compare_module_specifiers_for_sorting(a, b)
    }

    pub fn compare_fixes_for_ranking(&self, a: &Fix, b: &Fix) -> i32 {
        let ranking = compare_fix_kinds(a.kind(), b.kind());
        if ranking != 0 {
            return ranking;
        }
        self.compare_module_specifiers_for_ranking(a, b)
    }

    pub fn compare_module_specifiers_for_ranking(&self, a: &Fix, b: &Fix) -> i32 {
        let comparison = compare_module_specifier_relativity(
            a,
            b,
            self.preferences.module_specifier_preferences(),
        );
        if comparison != 0 {
            return comparison;
        }
        if a.module_specifier_kind == Some(modulespecifiers::ResultKind::Ambient)
            && b.module_specifier_kind == Some(modulespecifiers::ResultKind::Ambient)
        {
            let Some(importing_file) = self.importing_file.as_ref() else {
                return 0;
            };
            let Some(program) = self.program else {
                return 0;
            };
            let comparison = self.compare_node_core_module_specifiers(
                a.module_specifier(),
                b.module_specifier(),
                importing_file,
                program,
            );
            if comparison != 0 {
                return comparison;
            }
        }
        if a.module_specifier_kind == Some(modulespecifiers::ResultKind::Relative)
            && b.module_specifier_kind == Some(modulespecifiers::ResultKind::Relative)
        {
            let importing_file_name = self
                .importing_file
                .as_ref()
                .map(|file| file.file_name().to_string())
                .unwrap_or_default();
            let re_export_cmp = core::compare_booleans(
                is_fix_possibly_re_exporting_importing_file(a, &importing_file_name),
                is_fix_possibly_re_exporting_importing_file(b, &importing_file_name),
            );
            if re_export_cmp != 0 {
                return re_export_cmp;
            }
        }
        match tspath::compare_number_of_directory_separators(
            a.module_specifier(),
            b.module_specifier(),
        ) {
            cmp::Ordering::Less => -1,
            cmp::Ordering::Equal => 0,
            cmp::Ordering::Greater => 1,
        }
    }

    pub fn compare_module_specifiers_for_sorting(&self, a: &Fix, b: &Fix) -> i32 {
        let ranking = self.compare_module_specifiers_for_ranking(a, b);
        if ranking != 0 {
            return ranking;
        }
        if a.module_specifier().starts_with("./") && !b.module_specifier().starts_with("./") {
            return -1;
        }
        if b.module_specifier().starts_with("./") && !a.module_specifier().starts_with("./") {
            return 1;
        }
        let comparison = a.module_specifier().cmp(b.module_specifier());
        if comparison != cmp::Ordering::Equal {
            return match comparison {
                cmp::Ordering::Less => -1,
                cmp::Ordering::Equal => 0,
                cmp::Ordering::Greater => 1,
            };
        }
        match a.import_kind().cmp(&b.import_kind()) {
            cmp::Ordering::Less => -1,
            cmp::Ordering::Equal => 0,
            cmp::Ordering::Greater => 1,
        }
    }

    pub fn compare_node_core_module_specifiers(
        &self,
        a: &str,
        b: &str,
        _importing_file: &ast::SourceFile,
        _program: &compiler::Program,
    ) -> i32 {
        if a.starts_with("node:") && !b.starts_with("node:") {
            if self.should_use_uri_style_node_core_modules.is_true() {
                return -1;
            } else if self.should_use_uri_style_node_core_modules.is_false() {
                return 1;
            }
            return 0;
        }
        if b.starts_with("node:") && !a.starts_with("node:") {
            if self.should_use_uri_style_node_core_modules.is_true() {
                return 1;
            } else if self.should_use_uri_style_node_core_modules.is_false() {
                return -1;
            }
        }
        0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FileSyntaxKind {
    #[default]
    Ambiguous = 0,
    Esm = 1,
    Cjs = 2,
}

pub const FILE_SYNTAX_KIND_AMBIGUOUS: FileSyntaxKind = FileSyntaxKind::Ambiguous;
pub const FILE_SYNTAX_KIND_ESM: FileSyntaxKind = FileSyntaxKind::Esm;
pub const FILE_SYNTAX_KIND_CJS: FileSyntaxKind = FileSyntaxKind::Cjs;

pub fn get_add_as_type_only(
    is_valid_type_only_use_site: bool,
    export: &Export,
    compiler_options: &core::CompilerOptions,
) -> lsproto::AddAsTypeOnly {
    if !is_valid_type_only_use_site {
        return lsproto::AddAsTypeOnly::NotAllowed;
    }
    if compiler_options.verbatim_module_syntax.is_true()
        && (export.is_type_only || export.flags & ast::SYMBOL_FLAGS_VALUE == 0)
        || export.is_type_only && export.flags & ast::SYMBOL_FLAGS_VALUE != 0
    {
        return lsproto::AddAsTypeOnly::Required;
    }
    lsproto::AddAsTypeOnly::Allowed
}

pub(crate) fn get_namespace_like_import_text(
    store: &ast::AstStore,
    declaration: &ast::Node,
) -> String {
    match store.kind(*declaration) {
        ast::Kind::VariableDeclaration => {
            if let Some(name) = store.name(*declaration) {
                if store.kind(name) == ast::Kind::Identifier {
                    return store.text(name);
                }
            }
            String::new()
        }
        ast::Kind::ImportEqualsDeclaration => store.text(store.name(*declaration).unwrap()),
        ast::Kind::ImportDeclaration => {
            if let Some(import_clause) = store.import_clause(*declaration) {
                if let Some(named_bindings) = store.named_bindings(import_clause) {
                    if store.kind(named_bindings) == ast::Kind::NamespaceImport {
                        return store.text(store.name(named_bindings).unwrap());
                    }
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

pub fn get_import_kind(
    importing_file: &ast::SourceFile,
    export: &Export,
    program: &compiler::Program,
) -> lsproto::ImportKind {
    if program.compiler_options().verbatim_module_syntax.is_true()
        && program.get_emit_module_format_of_file_for_auto_imports(importing_file)
            == core::ModuleKind::CommonJS
    {
        return lsproto::ImportKind::CommonJS;
    }
    match export.syntax {
        ExportSyntax::DefaultModifier | ExportSyntax::DefaultDeclaration => {
            lsproto::ImportKind::Default
        }
        ExportSyntax::Named => {
            if export.export_name() == ast::INTERNAL_SYMBOL_NAME_DEFAULT {
                return lsproto::ImportKind::Default;
            }
            lsproto::ImportKind::Named
        }
        ExportSyntax::Modifier | ExportSyntax::Star | ExportSyntax::CommonJSExportsProperty => {
            lsproto::ImportKind::Named
        }
        ExportSyntax::Equals | ExportSyntax::CommonJSModuleExports | ExportSyntax::UMD => {
            if export.export_name() != ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS {
                return lsproto::ImportKind::Named;
            }
            let store = importing_file.store();
            for statement in importing_file.statements_view() {
                if ast::is_import_equals_declaration(store, statement)
                    && store
                        .module_reference(statement)
                        .is_some_and(|module_reference| {
                            !ast::node_is_missing(store, Some(module_reference))
                        })
                {
                    return lsproto::ImportKind::CommonJS;
                }
            }
            if importing_file.external_module_indicator().is_some()
                || !ast::is_source_file_js(importing_file)
            {
                return lsproto::ImportKind::Default;
            }
            lsproto::ImportKind::CommonJS
        }
        ExportSyntax::None => panic!("unhandled export syntax kind: {}", export.syntax),
    }
}

pub fn detect_syntax(file: &ast::SourceFile, options: &core::CompilerOptions) -> FileSyntaxKind {
    let (has_esm, has_cjs) = detect_syntax_indicators(file, options);
    match (has_esm, has_cjs) {
        (false, true) => FileSyntaxKind::Cjs,
        (true, false) => FileSyntaxKind::Esm,
        _ => FileSyntaxKind::Ambiguous,
    }
}

pub fn detect_syntax_indicators(
    file: &ast::SourceFile,
    options: &core::CompilerOptions,
) -> (bool, bool) {
    let has_cjs = file.common_js_module_indicator().is_some();
    if options.get_emit_module_detection_kind() != core::ModuleDetectionKind::Force {
        let has_esm = file.external_module_indicator().is_some();
        return (has_esm, has_cjs);
    }
    if file.external_module_indicator().is_some()
        && file
            .external_module_indicator()
            .is_some_and(|indicator| indicator != file.as_node())
    {
        return (true, has_cjs);
    }
    let store = file.store();
    for imp in file.imports() {
        if store.flags(*imp).intersects(ast::NodeFlags::Synthesized) {
            continue;
        }
        let Some(parent) = store.parent(*imp) else {
            continue;
        };
        match store.kind(parent) {
            ast::Kind::ImportDeclaration
            | ast::Kind::JSImportDeclaration
            | ast::Kind::ExportDeclaration => return (true, has_cjs),
            ast::Kind::ExternalModuleReference => return (true, has_cjs),
            _ => {}
        }
    }
    (false, has_cjs)
}

pub fn needs_type_only(add_as_type_only: lsproto::AddAsTypeOnly) -> bool {
    add_as_type_only == lsproto::AddAsTypeOnly::Required
}

pub fn should_use_type_only(
    add_as_type_only: lsproto::AddAsTypeOnly,
    preferences: lsutil::UserPreferences,
) -> bool {
    needs_type_only(add_as_type_only)
        || add_as_type_only != lsproto::AddAsTypeOnly::NotAllowed
            && preferences.prefer_type_only_auto_imports.is_true()
}

pub fn compare_fix_kinds(
    a: Option<lsproto::AutoImportFixKind>,
    b: Option<lsproto::AutoImportFixKind>,
) -> i32 {
    match (a, b) {
        (Some(a), Some(b)) => a.0 - b.0,
        (None, Some(_)) => -1,
        (Some(_), None) => 1,
        (None, None) => 0,
    }
}

pub fn is_fix_possibly_re_exporting_importing_file(fix: &Fix, importing_file_name: &str) -> bool {
    if fix.is_re_export && is_index_file_name(&fix.module_file_name) {
        let re_export_dir = tspath::get_directory_path(&fix.module_file_name);
        return importing_file_name.starts_with(&re_export_dir);
    }
    false
}

pub fn is_index_file_name(file_name: &str) -> bool {
    let Some((_, file_name)) = file_name.rsplit_once('/') else {
        return false;
    };
    matches!(
        file_name,
        "index.js" | "index.jsx" | "index.d.ts" | "index.ts" | "index.tsx"
    )
}

pub fn promote_from_type_only<'a>(
    changes: &mut change::Tracker<'a>,
    alias_declaration: &ast::Node,
    compiler_options: &core::CompilerOptions,
    source_file: &'a ast::SourceFile,
    preferences: lsutil::UserPreferences,
) -> Option<ast::Node> {
    // See comment in `doAddExistingFix` on constant with the same name.
    let convert_existing_to_type_only = compiler_options.verbatim_module_syntax;
    let store = source_file.store();

    match store.kind(*alias_declaration) {
        ast::Kind::ImportSpecifier => {
            if store.is_type_only(*alias_declaration).unwrap_or(false) {
                // If no re-sorting needed, just remove the 'type' keyword
                delete_type_keyword(changes, source_file, store.loc(*alias_declaration).pos());
                Some(alias_declaration.clone())
            } else {
                // The parent import clause is type-only
                let parent = store
                    .parent(*alias_declaration)
                    .expect("ImportSpecifier should have NamedImports parent");
                if store.kind(parent) != ast::Kind::NamedImports {
                    panic!("ImportSpecifier parent must be NamedImports");
                }
                let import_clause = store
                    .parent(parent)
                    .expect("NamedImports should have ImportClause parent");
                if store.kind(import_clause) != ast::Kind::ImportClause {
                    panic!("NamedImports parent must be ImportClause");
                }
                promote_import_clause(
                    changes,
                    import_clause,
                    compiler_options,
                    source_file,
                    preferences,
                    convert_existing_to_type_only,
                    Some(alias_declaration),
                );
                Some(import_clause)
            }
        }
        ast::Kind::ImportClause => {
            promote_import_clause(
                changes,
                *alias_declaration,
                compiler_options,
                source_file,
                preferences,
                convert_existing_to_type_only,
                Some(alias_declaration),
            );
            Some(alias_declaration.clone())
        }
        ast::Kind::NamespaceImport => {
            // Promote the parent import clause
            let parent = store
                .parent(*alias_declaration)
                .expect("NamespaceImport should have ImportClause parent");
            if store.kind(parent) != ast::Kind::ImportClause {
                panic!("NamespaceImport parent must be ImportClause");
            }
            promote_import_clause(
                changes,
                parent,
                compiler_options,
                source_file,
                preferences,
                convert_existing_to_type_only,
                Some(alias_declaration),
            );
            Some(parent)
        }
        ast::Kind::ImportEqualsDeclaration => {
            // Remove the 'type' keyword (which is the second token: 'import' 'type' name '=' ...)
            delete_type_keyword(changes, source_file, store.loc(*alias_declaration).pos());
            Some(alias_declaration.clone())
        }
        _ => panic!(
            "Unexpected alias declaration kind: {}",
            store.kind(*alias_declaration)
        ),
    }
}

pub fn promote_import_clause<'a>(
    changes: &mut change::Tracker<'a>,
    import_clause_node: ast::Node,
    compiler_options: &core::CompilerOptions,
    source_file: &'a ast::SourceFile,
    _preferences: lsutil::UserPreferences,
    convert_existing_to_type_only: core::Tristate,
    alias_declaration: Option<&ast::Node>,
) {
    let store = source_file.store();
    // Delete the 'type' keyword
    if store.phase_modifier(import_clause_node) == Some(ast::Kind::TypeKeyword) {
        delete_type_keyword(changes, source_file, store.loc(import_clause_node).pos());
    }

    // Handle .ts extension conversion to .js if necessary
    if compiler_options.allow_importing_ts_extensions.is_false() {
        // Note: We can't check ResolvedUsingTsExtension without program, so we'll skip this optimization
        // The fix will still work, just might not change .ts to .js extensions in all cases
    }

    // Handle verbatimModuleSyntax conversion
    // If convertExistingToTypeOnly is true, we need to add 'type' to other specifiers
    // in the same import declaration
    if convert_existing_to_type_only.is_true() {
        if let Some(named_imports) = store.named_bindings(import_clause_node) {
            if store.kind(named_imports) == ast::Kind::NamedImports {
                if store
                    .elements(named_imports)
                    .is_some_and(|elements| elements.len() > 1)
                {
                    // Add 'type' keyword to all other import specifiers that aren't already type-only
                    let Some(elements) = store.elements(named_imports) else {
                        return;
                    };
                    for element in elements {
                        // Skip the specifier being promoted (if aliasDeclaration is an ImportSpecifier)
                        if alias_declaration.is_some_and(|alias_declaration| {
                            store.kind(*alias_declaration) == ast::Kind::ImportSpecifier
                                && store.loc(*alias_declaration).pos() == store.loc(element).pos()
                                && store.loc(*alias_declaration).end() == store.loc(element).end()
                        }) {
                            continue;
                        }
                        // Skip if already type-only
                        if !store.is_type_only(element).unwrap_or(false) {
                            insert_text_at_pos(
                                changes,
                                source_file,
                                store.loc(element).pos(),
                                "type ",
                            );
                        }
                    }
                }
            }
        }
    }
}

pub fn delete_type_keyword<'a>(
    changes: &mut change::Tracker<'a>,
    source_file: &'a ast::SourceFile,
    start_pos: i32,
) {
    // deleteTypeKeyword deletes the 'type' keyword token starting at the given position,
    // including any trailing whitespace.
    let text = source_file.text().as_bytes();
    let mut type_start = start_pos.max(0) as usize;
    while type_start < text.len() && matches!(text[type_start], b' ' | b'\t' | b'\r' | b'\n') {
        type_start += 1;
    }
    if text.get(type_start..type_start + 6) == Some(b"import") {
        type_start += 6;
        while type_start < text.len() && matches!(text[type_start], b' ' | b'\t' | b'\r' | b'\n') {
            type_start += 1;
        }
    }
    if text.get(type_start..type_start + 4) != Some(b"type") {
        return;
    }
    let before_is_boundary = type_start == 0 || !is_identifier_byte(text[type_start - 1]);
    let after_is_boundary =
        type_start + 4 >= text.len() || !is_identifier_byte(text[type_start + 4]);
    if !before_is_boundary || !after_is_boundary {
        return;
    }
    let mut type_end = type_start + 4;
    // Skip trailing whitespace
    while type_end < text.len() && matches!(text[type_end], b' ' | b'\t') {
        type_end += 1;
    }
    changes.delete_range(
        source_file,
        core::new_text_range(type_start as i32, type_end as i32),
    );
}

fn is_identifier_byte(byte: u8) -> bool {
    byte == b'_' || byte == b'$' || byte.is_ascii_alphanumeric()
}

pub fn get_module_specifier_text(
    store: &ast::AstStore,
    promoted_declaration: &ast::Node,
) -> String {
    if store.kind(*promoted_declaration) == ast::Kind::ImportEqualsDeclaration {
        if let Some(module_reference) = store.module_reference(*promoted_declaration) {
            if ast::is_external_module_reference(store, module_reference) {
                if let Some(expr) = store.expression(module_reference) {
                    if store.kind(expr) == ast::Kind::StringLiteral {
                        return store.text(expr);
                    }
                }
            }
        }
        return store
            .module_reference(*promoted_declaration)
            .map(|module_reference| store.text(module_reference))
            .unwrap_or_default();
    }
    let parent = store.parent(*promoted_declaration).unwrap();
    store.text(store.module_specifier(parent).unwrap())
}

pub fn compare_module_specifier_relativity(
    a: &Fix,
    b: &Fix,
    preferences: modulespecifiers::UserPreferences,
) -> i32 {
    match preferences.import_module_specifier_preference {
        modulespecifiers::ImportModuleSpecifierPreference::NonRelative
        | modulespecifiers::ImportModuleSpecifierPreference::ProjectRelative => {
            core::compare_booleans(
                a.module_specifier_kind == Some(modulespecifiers::ResultKind::Relative),
                b.module_specifier_kind == Some(modulespecifiers::ResultKind::Relative),
            )
        }
        _ => 0,
    }
}

impl fmt::Display for FileSyntaxKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileSyntaxKind::Ambiguous => f.write_str("fileSyntaxKindAmbiguous"),
            FileSyntaxKind::Esm => f.write_str("fileSyntaxKindESM"),
            FileSyntaxKind::Cjs => f.write_str("fileSyntaxKindCJS"),
        }
    }
}
