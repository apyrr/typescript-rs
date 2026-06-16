use std::cmp::Ordering;

use ts_ast as ast;
use ts_core as core;
use ts_locale as locale;
use ts_stringutil as stringutil;
use ts_tspath as tspath;

use crate::lsutil::{
    OrganizeImportsCaseFirst, OrganizeImportsCollation, OrganizeImportsTypeOrder, UserPreferences,
};

pub fn case_insensitive_organize_imports_comparer() -> Vec<fn(&str, &str) -> i32> {
    vec![get_organize_imports_ordinal_string_comparer(true)]
}

pub fn case_sensitive_organize_imports_comparer() -> Vec<fn(&str, &str) -> i32> {
    vec![get_organize_imports_ordinal_string_comparer(false)]
}

pub fn organize_imports_comparers() -> Vec<fn(&str, &str) -> i32> {
    vec![
        case_insensitive_organize_imports_comparer()[0],
        case_sensitive_organize_imports_comparer()[0],
    ]
}

// FilterImportDeclarations filters out non-import declarations from a list of statements.
pub fn filter_import_declarations<'a>(
    store: &ast::AstStore,
    statements: &'a [ast::Statement],
) -> Vec<ast::Statement> {
    statements
        .iter()
        .copied()
        .filter(|stmt| store.kind(*stmt) == ast::Kind::ImportDeclaration)
        .collect()
}

// GetDetectionLists returns the lists of comparers and type orders to test for organize imports detection.
pub fn get_detection_lists(
    preferences: UserPreferences,
) -> (
    Vec<Box<dyn Fn(&str, &str) -> i32>>,
    Vec<OrganizeImportsTypeOrder>,
) {
    let comparers_to_test: Vec<Box<dyn Fn(&str, &str) -> i32>> =
        if !preferences.organize_imports_ignore_case.is_unknown() {
            let ignore_case = preferences.organize_imports_ignore_case.is_true();
            vec![Box::new(get_organize_imports_string_comparer(
                preferences.clone(),
                ignore_case,
            ))]
        } else {
            vec![
                Box::new(get_organize_imports_string_comparer(
                    preferences.clone(),
                    true,
                )),
                Box::new(get_organize_imports_string_comparer(
                    preferences.clone(),
                    false,
                )),
            ]
        };

    let type_orders_to_test =
        if preferences.organize_imports_type_order != OrganizeImportsTypeOrder::Auto {
            vec![preferences.organize_imports_type_order]
        } else {
            vec![
                OrganizeImportsTypeOrder::Last,
                OrganizeImportsTypeOrder::Inline,
                OrganizeImportsTypeOrder::First,
            ]
        };

    (comparers_to_test, type_orders_to_test)
}

pub fn get_organize_imports_ordinal_string_comparer(ignore_case: bool) -> fn(&str, &str) -> i32 {
    if ignore_case {
        stringutil::compare_strings_case_insensitive_eslint_compatible
    } else {
        stringutil::compare_strings_case_sensitive
    }
}

pub fn get_organize_imports_unicode_string_comparer(
    ignore_case: bool,
    preferences: UserPreferences,
) -> impl Fn(&str, &str) -> i32 {
    let _resolved_locale = get_organize_imports_locale(preferences.clone());
    let case_first = preferences.organize_imports_case_first;
    let numeric = preferences.organize_imports_numeric_collation.is_true();
    let accents = !preferences.organize_imports_accent_collation.is_false();

    move |a: &str, b: &str| {
        let a_key = if ignore_case {
            a.to_lowercase()
        } else {
            a.to_string()
        };
        let b_key = if ignore_case {
            b.to_lowercase()
        } else {
            b.to_string()
        };

        let mut primary_cmp = a_key.cmp(&b_key);
        if !accents && primary_cmp == Ordering::Equal {
            primary_cmp = a.len().cmp(&b.len());
        }
        if primary_cmp != Ordering::Equal {
            return match primary_cmp {
                Ordering::Less => -1,
                Ordering::Greater => 1,
                Ordering::Equal => 0,
            };
        }

        let a_runes = a.chars().collect::<Vec<_>>();
        let b_runes = b.chars().collect::<Vec<_>>();
        let min_len = a_runes.len().min(b_runes.len());
        for i in 0..min_len {
            let a_upper = a_runes[i].is_uppercase();
            let b_upper = b_runes[i].is_uppercase();
            if a_upper != b_upper {
                return match case_first {
                    OrganizeImportsCaseFirst::Upper => {
                        if a_upper {
                            -1
                        } else {
                            1
                        }
                    }
                    OrganizeImportsCaseFirst::Lower => {
                        if !a_upper {
                            -1
                        } else {
                            1
                        }
                    }
                    OrganizeImportsCaseFirst::False => {
                        if a_upper {
                            1
                        } else {
                            -1
                        }
                    }
                };
            }
        }

        if numeric {
            return get_organize_imports_ordinal_string_comparer(ignore_case)(a, b);
        }
        0
    }
}

pub fn get_organize_imports_locale(preferences: UserPreferences) -> String {
    let mut locale_str = "en".to_string();
    if !preferences.organize_imports_locale.is_empty() {
        locale_str = preferences.organize_imports_locale.clone();
    }

    if locale_str == "auto" {
        if !locale::DEFAULT.is_und() {
            return locale::DEFAULT.as_str().to_string();
        }
        return "en".to_string();
    }

    let (parsed, ok) = locale::parse(&locale_str);
    if ok {
        return parsed.as_str().to_string();
    }

    "en".to_string()
}

pub fn get_organize_imports_string_comparer(
    preferences: UserPreferences,
    ignore_case: bool,
) -> impl Fn(&str, &str) -> i32 {
    let collation = preferences.organize_imports_collation;
    move |a: &str, b: &str| {
        if collation == OrganizeImportsCollation::Unicode {
            return get_organize_imports_unicode_string_comparer(ignore_case, preferences.clone())(
                a, b,
            );
        }
        get_organize_imports_ordinal_string_comparer(ignore_case)(a, b)
    }
}

pub fn get_module_specifier_expression(
    store: &ast::AstStore,
    declaration: &ast::Statement,
) -> Option<ast::Expression> {
    match store.kind(*declaration) {
        ast::Kind::ImportEqualsDeclaration => {
            if let Some(module_reference) = store.module_reference(*declaration)
                && store.kind(module_reference) == ast::Kind::ExternalModuleReference
            {
                return store.expression(module_reference);
            }
            None
        }
        ast::Kind::ImportDeclaration => store.module_specifier(*declaration),
        ast::Kind::VariableStatement => {
            let declaration_list_node = store.declaration_list(*declaration).unwrap();
            let declarations = store.declarations(declaration_list_node).unwrap();
            if let Some(first_declaration) = declarations.first() {
                let initializer = store.initializer(first_declaration);
                if initializer
                    .is_some_and(|initializer| ast::is_call_expression(store, initializer))
                {
                    let arguments = store.arguments(initializer.unwrap()).unwrap();
                    if let Some(first_argument) = arguments.first() {
                        return Some(first_argument);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

// GetExternalModuleName returns the module name from a module specifier expression.
pub fn get_external_module_name(
    store: &ast::AstStore,
    specifier: Option<&ast::Expression>,
) -> String {
    if let Some(specifier) = specifier {
        if ast::is_string_literal_like(store, *specifier) {
            return store.text(*specifier);
        }
    }
    String::new()
}

// CompareModuleSpecifiers compares two module specifiers using the given comparer.
pub fn compare_module_specifiers(
    store: &ast::AstStore,
    m1: Option<&ast::Expression>,
    m2: Option<&ast::Expression>,
    comparer: impl Fn(&str, &str) -> i32,
) -> i32 {
    let name1 = get_external_module_name(store, m1);
    let name2 = get_external_module_name(store, m2);
    let cmp = core::compare_booleans(name1.is_empty(), name2.is_empty());
    if cmp != 0 {
        return cmp;
    }
    let cmp = core::compare_booleans(
        tspath::is_external_module_name_relative(&name1),
        tspath::is_external_module_name_relative(&name2),
    );
    if cmp != 0 {
        return cmp;
    }
    comparer(&name1, &name2)
}

pub fn compare_import_kind(store: &ast::AstStore, s1: &ast::Statement, s2: &ast::Statement) -> i32 {
    match get_import_kind_order(store, s1).cmp(&get_import_kind_order(store, s2)) {
        Ordering::Less => -1,
        Ordering::Greater => 1,
        Ordering::Equal => 0,
    }
}

const IMPORT_KIND_ORDER_SIDE_EFFECT: i32 = 0;
const IMPORT_KIND_ORDER_TYPE_ONLY: i32 = 1;
const IMPORT_KIND_ORDER_NAMESPACE: i32 = 2;
const IMPORT_KIND_ORDER_DEFAULT: i32 = 3;
const IMPORT_KIND_ORDER_NAMED: i32 = 4;
const IMPORT_KIND_ORDER_IMPORT_EQUALS: i32 = 5;
const IMPORT_KIND_ORDER_REQUIRE: i32 = 6;
const IMPORT_KIND_ORDER_UNKNOWN: i32 = 7;

pub fn get_import_kind_order(store: &ast::AstStore, s1: &ast::Statement) -> i32 {
    match store.kind(*s1) {
        ast::Kind::ImportDeclaration => {
            let Some(import_clause) = store.import_clause(*s1) else {
                return IMPORT_KIND_ORDER_SIDE_EFFECT;
            };
            if store.is_type_only(import_clause).unwrap_or(false) {
                return IMPORT_KIND_ORDER_TYPE_ONLY;
            }
            if store
                .named_bindings(import_clause)
                .is_some_and(|named_bindings| ast::is_namespace_import(store, named_bindings))
            {
                return IMPORT_KIND_ORDER_NAMESPACE;
            }
            if store.name(import_clause).is_some() {
                return IMPORT_KIND_ORDER_DEFAULT;
            }
            IMPORT_KIND_ORDER_NAMED
        }
        ast::Kind::ImportEqualsDeclaration => IMPORT_KIND_ORDER_IMPORT_EQUALS,
        ast::Kind::VariableStatement => IMPORT_KIND_ORDER_REQUIRE,
        _ => IMPORT_KIND_ORDER_UNKNOWN,
    }
}

// CompareImportsOrRequireStatements compares two import or require statements.
pub fn compare_imports_or_require_statements(
    store: &ast::AstStore,
    s1: &ast::Statement,
    s2: &ast::Statement,
    comparer: impl Fn(&str, &str) -> i32 + Copy,
) -> i32 {
    let m1 = get_module_specifier_expression(store, s1);
    let m2 = get_module_specifier_expression(store, s2);
    let cmp = compare_module_specifiers(store, m1.as_ref(), m2.as_ref(), comparer);
    if cmp != 0 {
        return cmp;
    }
    compare_import_kind(store, s1, s2)
}

pub fn compare_import_or_export_specifiers(
    store: &ast::AstStore,
    s1: &ast::Node,
    s2: &ast::Node,
    comparer: impl Fn(&str, &str) -> i32,
    preferences: UserPreferences,
) -> i32 {
    let type_order = preferences.organize_imports_type_order;
    let s1_name = store.text(store.name(*s1).unwrap());
    let s2_name = store.text(store.name(*s2).unwrap());

    match type_order {
        OrganizeImportsTypeOrder::First => {
            let cmp = core::compare_booleans(
                store.is_type_only(*s2).unwrap_or(false),
                store.is_type_only(*s1).unwrap_or(false),
            );
            if cmp != 0 {
                return cmp;
            }
            comparer(&s1_name, &s2_name)
        }
        OrganizeImportsTypeOrder::Inline => comparer(&s1_name, &s2_name),
        OrganizeImportsTypeOrder::Last | OrganizeImportsTypeOrder::Auto => {
            let cmp = core::compare_booleans(
                store.is_type_only(*s1).unwrap_or(false),
                store.is_type_only(*s2).unwrap_or(false),
            );
            if cmp != 0 {
                return cmp;
            }
            comparer(&s1_name, &s2_name)
        }
    }
}

// GetNamedImportSpecifierComparer returns a comparer function for sorting import specifiers.
pub fn get_named_import_specifier_comparer(
    store: &ast::AstStore,
    preferences: UserPreferences,
    comparer: Option<fn(&str, &str) -> i32>,
) -> impl Fn(&ast::Node, &ast::Node) -> i32 + '_ {
    let comparer = comparer.unwrap_or_else(|| {
        let mut ignore_case = false;
        if !preferences.organize_imports_ignore_case.is_unknown() {
            ignore_case = preferences.organize_imports_ignore_case.is_true();
        }
        get_organize_imports_ordinal_string_comparer(ignore_case)
    });
    move |s1: &ast::Node, s2: &ast::Node| {
        compare_import_or_export_specifiers(store, s1, s2, comparer, preferences.clone())
    }
}

// GetImportSpecifierInsertionIndex returns the index at which to insert a new import specifier.
pub fn get_import_specifier_insertion_index(
    sorted_imports: &[&ast::Node],
    new_import: &ast::Node,
    comparer: impl Fn(&ast::Node, &ast::Node) -> i32,
) -> usize {
    let (index, found) = core::binary_search_unique_func(sorted_imports, |_mid, value| {
        match comparer(value, new_import) {
            x if x < 0 => Ordering::Less,
            x if x > 0 => Ordering::Greater,
            _ => Ordering::Equal,
        }
    });
    if found { index } else { index }
}

// GetImportDeclarationInsertIndex returns the index at which to insert a new import declaration.
pub fn get_import_declaration_insert_index(
    sorted_imports: &[&ast::Statement],
    new_import: &ast::Statement,
    comparer: impl Fn(&ast::Statement, &ast::Statement) -> i32,
) -> usize {
    let (index, found) = core::binary_search_unique_func(sorted_imports, |_mid, value| {
        match comparer(value, new_import) {
            x if x < 0 => Ordering::Less,
            x if x > 0 => Ordering::Greater,
            _ => Ordering::Equal,
        }
    });
    if found { index } else { index }
}

// GetOrganizeImportsStringComparerWithDetection returns a string comparer based on detecting the order of import statements by the module specifier
pub fn get_organize_imports_string_comparer_with_detection(
    store: &ast::AstStore,
    original_import_decls: &[ast::Statement],
    preferences: UserPreferences,
) -> (Box<dyn Fn(&str, &str) -> i32>, bool) {
    let result = detect_module_specifier_case_by_sort(
        store,
        vec![original_import_decls.to_vec()],
        get_comparers(preferences),
    );
    result
}

pub fn get_comparers(preferences: UserPreferences) -> Vec<fn(&str, &str) -> i32> {
    match preferences.organize_imports_ignore_case {
        core::Tristate::True => case_insensitive_organize_imports_comparer(),
        core::Tristate::False => case_sensitive_organize_imports_comparer(),
        _ => organize_imports_comparers(),
    }
}

#[derive(Default)]
pub struct NamedImportSortResult {
    pub named_import_comparer: Option<fn(&str, &str) -> i32>,
    pub type_order: OrganizeImportsTypeOrder,
    pub is_sorted: bool,
}

// DetectNamedImportOrganizationBySort detects the order of named imports throughout the file by considering the named imports in each statement as a group
pub fn detect_named_import_organization_by_sort(
    store: &ast::AstStore,
    original_groups: &[ast::Statement],
    comparers_to_test: &[fn(&str, &str) -> i32],
    types_to_test: &[OrganizeImportsTypeOrder],
) -> (
    Option<fn(&str, &str) -> i32>,
    OrganizeImportsTypeOrder,
    bool,
) {
    let result = detect_named_import_organization_by_sort_worker(
        store,
        original_groups,
        comparers_to_test,
        types_to_test,
    );
    if let Some(result) = result {
        return (result.named_import_comparer, result.type_order, true);
    }
    (None, OrganizeImportsTypeOrder::Last, false)
}

pub fn detect_named_import_organization_by_sort_worker(
    store: &ast::AstStore,
    original_groups: &[ast::Statement],
    comparers_to_test: &[fn(&str, &str) -> i32],
    types_to_test: &[OrganizeImportsTypeOrder],
) -> Option<NamedImportSortResult> {
    let comparer_refs = comparers_to_test
        .iter()
        .map(|comparer| comparer as &dyn Fn(&str, &str) -> i32)
        .collect::<Vec<_>>();
    let result = detect_named_import_organization_by_sort_worker_for_comparers(
        store,
        original_groups,
        &comparer_refs,
        types_to_test,
    )?;
    Some(NamedImportSortResult {
        named_import_comparer: result
            .named_import_comparer_index
            .map(|index| comparers_to_test[index]),
        type_order: result.type_order,
        is_sorted: result.is_sorted,
    })
}

#[derive(Default)]
struct NamedImportSortSelection {
    named_import_comparer_index: Option<usize>,
    type_order: OrganizeImportsTypeOrder,
    is_sorted: bool,
}

fn detect_named_import_organization_by_sort_worker_for_comparers(
    store: &ast::AstStore,
    original_groups: &[ast::Statement],
    comparers_to_test: &[&dyn Fn(&str, &str) -> i32],
    types_to_test: &[OrganizeImportsTypeOrder],
) -> Option<NamedImportSortSelection> {
    let mut both_named_imports = false;
    let mut import_decls_with_named = Vec::new();

    for imp in original_groups {
        let Some(import_clause) = store.import_clause(*imp) else {
            continue;
        };
        let Some(named_bindings) = store.named_bindings(import_clause) else {
            continue;
        };
        if store.kind(named_bindings) != ast::Kind::NamedImports {
            continue;
        }
        let elements = store.elements(named_bindings).unwrap();
        if elements.is_empty() {
            continue;
        }

        if !both_named_imports {
            let mut has_type_only = false;
            let mut has_regular = false;
            for elem in elements {
                if store.is_type_only(elem).unwrap_or(false) {
                    has_type_only = true;
                } else {
                    has_regular = true;
                }
            }
            if has_type_only && has_regular {
                both_named_imports = true;
            }
        }

        import_decls_with_named.push(*imp);
    }

    if import_decls_with_named.is_empty() {
        return None;
    }

    let mut named_imports_by_decl: Vec<Vec<ast::Node>> = Vec::new();
    for imp in &import_decls_with_named {
        let clause_node = store.import_clause(*imp).unwrap();
        let named_imports = store.named_bindings(clause_node).unwrap();
        named_imports_by_decl.push(store.elements(named_imports).unwrap().into_iter().collect());
    }

    if !both_named_imports || types_to_test.is_empty() {
        let names_list: Vec<Vec<String>> = named_imports_by_decl
            .iter()
            .map(|imports| {
                imports
                    .iter()
                    .map(|imp| store.text(store.name(*imp).unwrap()))
                    .collect()
            })
            .collect();
        let sort_state =
            detect_case_sensitivity_by_sort_for_comparers(names_list, comparers_to_test);
        let mut type_order = OrganizeImportsTypeOrder::Last;
        if types_to_test.len() == 1 {
            type_order = types_to_test[0];
        }
        return Some(NamedImportSortSelection {
            named_import_comparer_index: sort_state.comparer_index,
            type_order,
            is_sorted: sort_state.is_sorted,
        });
    }

    let mut best_diff = std::collections::HashMap::from([
        (OrganizeImportsTypeOrder::First, i32::MAX),
        (OrganizeImportsTypeOrder::Last, i32::MAX),
        (OrganizeImportsTypeOrder::Inline, i32::MAX),
    ]);
    let mut best_comparer = std::collections::HashMap::from([
        (OrganizeImportsTypeOrder::First, 0usize),
        (OrganizeImportsTypeOrder::Last, 0usize),
        (OrganizeImportsTypeOrder::Inline, 0usize),
    ]);

    for (cur_comparer_index, cur_comparer) in comparers_to_test.iter().enumerate() {
        let mut curr_diff = std::collections::HashMap::from([
            (OrganizeImportsTypeOrder::First, 0),
            (OrganizeImportsTypeOrder::Last, 0),
            (OrganizeImportsTypeOrder::Inline, 0),
        ]);

        for import_decl in &named_imports_by_decl {
            for type_order in types_to_test {
                let prefs = UserPreferences {
                    organize_imports_type_order: *type_order,
                    ..UserPreferences::default()
                };
                let diff = measure_sortedness(import_decl, |n1, n2| {
                    compare_import_or_export_specifiers(
                        store,
                        n1,
                        n2,
                        |a, b| cur_comparer(a, b),
                        prefs.clone(),
                    )
                });
                *curr_diff.get_mut(type_order).unwrap() += diff;
            }
        }

        for type_order in types_to_test {
            if curr_diff[type_order] < best_diff[type_order] {
                best_diff.insert(*type_order, curr_diff[type_order]);
                best_comparer.insert(*type_order, cur_comparer_index);
            }
        }
    }

    for best_type_order in types_to_test {
        let mut is_best = true;
        for test_type_order in types_to_test {
            if best_diff[test_type_order] < best_diff[best_type_order] {
                is_best = false;
                break;
            }
        }
        if is_best {
            return Some(NamedImportSortSelection {
                named_import_comparer_index: Some(best_comparer[best_type_order]),
                type_order: *best_type_order,
                is_sorted: best_diff[best_type_order] == 0,
            });
        }
    }

    Some(NamedImportSortSelection {
        named_import_comparer_index: Some(best_comparer[&OrganizeImportsTypeOrder::Last]),
        type_order: OrganizeImportsTypeOrder::Last,
        is_sorted: best_diff[&OrganizeImportsTypeOrder::Last] == 0,
    })
}

#[derive(Default)]
pub struct CaseSensitivityDetectionResult {
    pub comparer: Option<fn(&str, &str) -> i32>,
    pub is_sorted: bool,
}

// DetectModuleSpecifierCaseBySort detects the order of module specifiers based on import statements throughout the module/file
pub fn detect_module_specifier_case_by_sort(
    store: &ast::AstStore,
    import_decls_by_group: Vec<Vec<ast::Statement>>,
    comparers_to_test: Vec<fn(&str, &str) -> i32>,
) -> (Box<dyn Fn(&str, &str) -> i32>, bool) {
    let mut module_specifiers_by_group = Vec::new();
    for import_group in import_decls_by_group {
        let mut module_names = Vec::new();
        for decl in import_group {
            if let Some(expr) = get_module_specifier_expression(store, &decl) {
                module_names.push(get_external_module_name(store, Some(&expr)));
            } else {
                module_names.push(String::new());
            }
        }
        module_specifiers_by_group.push(module_names);
    }
    let result = detect_case_sensitivity_by_sort(module_specifiers_by_group, &comparers_to_test);
    let comparer = result.comparer.unwrap_or(comparers_to_test[0]);
    (Box::new(comparer), result.is_sorted)
}

pub fn detect_case_sensitivity_by_sort(
    original_groups: Vec<Vec<String>>,
    comparers_to_test: &[fn(&str, &str) -> i32],
) -> CaseSensitivityDetectionResult {
    let comparer_refs = comparers_to_test
        .iter()
        .map(|comparer| comparer as &dyn Fn(&str, &str) -> i32)
        .collect::<Vec<_>>();
    let result = detect_case_sensitivity_by_sort_for_comparers(original_groups, &comparer_refs);
    CaseSensitivityDetectionResult {
        comparer: result.comparer_index.map(|index| comparers_to_test[index]),
        is_sorted: result.is_sorted,
    }
}

#[derive(Default)]
struct CaseSensitivityDetectionSelection {
    comparer_index: Option<usize>,
    is_sorted: bool,
}

fn detect_case_sensitivity_by_sort_for_comparers(
    original_groups: Vec<Vec<String>>,
    comparers_to_test: &[&dyn Fn(&str, &str) -> i32],
) -> CaseSensitivityDetectionSelection {
    let mut best_comparer_index = None;
    let mut best_diff = i32::MAX;

    for (cur_comparer_index, cur_comparer) in comparers_to_test.iter().enumerate() {
        let mut diff_of_current_comparer = 0;
        for list_to_sort in &original_groups {
            if list_to_sort.len() <= 1 {
                continue;
            }
            diff_of_current_comparer += measure_sortedness(list_to_sort, |a, b| cur_comparer(a, b));
        }
        if diff_of_current_comparer < best_diff {
            best_diff = diff_of_current_comparer;
            best_comparer_index = Some(cur_comparer_index);
        }
    }

    if best_comparer_index.is_none() && !comparers_to_test.is_empty() {
        best_comparer_index = Some(0);
    }

    CaseSensitivityDetectionSelection {
        comparer_index: best_comparer_index,
        is_sorted: best_diff == 0,
    }
}

pub fn measure_sortedness<T>(arr: &[T], comparer: impl Fn(&T, &T) -> i32) -> i32 {
    let mut i = 0;
    if arr.len() <= 1 {
        return 0;
    }
    for j in 0..(arr.len() - 1) {
        if comparer(&arr[j], &arr[j + 1]) > 0 {
            i += 1;
        }
    }
    i
}

pub struct NamedImportSpecifierSorting {
    pub string_comparer: Box<dyn Fn(&str, &str) -> i32>,
    pub type_order: OrganizeImportsTypeOrder,
    pub is_sorted: core::Tristate,
}

pub fn get_named_import_specifier_sorting_with_detection(
    store: &ast::AstStore,
    import_decl: &ast::Node,
    source_file: Option<&ast::SourceFile>,
    preferences: UserPreferences,
) -> NamedImportSpecifierSorting {
    let (mut comparers_to_test, type_orders_to_test) = get_detection_lists(preferences.clone());
    let mut comparer_index = Some(0usize);
    let mut type_order = preferences.organize_imports_type_order;
    let mut is_sorted = core::Tristate::Unknown;

    let import_stmt = if store.kind(*import_decl) == ast::Kind::ImportDeclaration {
        Some(import_decl)
    } else {
        None
    };

    if (preferences.organize_imports_ignore_case.is_unknown()
        || preferences.organize_imports_type_order == OrganizeImportsTypeOrder::Auto)
        && import_stmt.is_some()
    {
        let detect_from_decl = {
            let comparer_refs = comparers_to_test
                .iter()
                .map(|comparer| comparer.as_ref() as &dyn Fn(&str, &str) -> i32)
                .collect::<Vec<_>>();
            detect_named_import_organization_by_sort_worker_for_comparers(
                store,
                &[*import_stmt.unwrap()],
                &comparer_refs,
                &type_orders_to_test,
            )
        };
        let detect_from_file = if detect_from_decl.is_none() {
            source_file.and_then(|source_file| {
                let statements: Vec<_> = source_file.statements_view().iter().collect();
                let all_imports = filter_import_declarations(store, &statements);
                let comparer_refs = comparers_to_test
                    .iter()
                    .map(|comparer| comparer.as_ref() as &dyn Fn(&str, &str) -> i32)
                    .collect::<Vec<_>>();
                detect_named_import_organization_by_sort_worker_for_comparers(
                    store,
                    &all_imports,
                    &comparer_refs,
                    &type_orders_to_test,
                )
            })
        } else {
            None
        };

        if let Some(detection) = detect_from_decl.or(detect_from_file) {
            is_sorted = core::bool_to_tristate(detection.is_sorted);
            comparer_index = detection.named_import_comparer_index;
            type_order = detection.type_order;
        }
    }

    let string_comparer = comparer_index
        .map(|index| comparers_to_test.remove(index))
        .unwrap_or_else(|| Box::new(get_organize_imports_ordinal_string_comparer(false)));

    NamedImportSpecifierSorting {
        string_comparer,
        type_order,
        is_sorted,
    }
}

// GetNamedImportSpecifierComparerWithDetection returns a specifier comparer based on detecting the existing sort order within a single import statement
pub fn get_named_import_specifier_comparer_with_detection<'a>(
    store: &'a ast::AstStore,
    import_decl: &'a ast::Node,
    source_file: Option<&ast::SourceFile>,
    preferences: UserPreferences,
) -> (
    Box<dyn Fn(&ast::Node, &ast::Node) -> i32 + 'a>,
    core::Tristate,
) {
    let sorting = get_named_import_specifier_sorting_with_detection(
        store,
        import_decl,
        source_file,
        preferences,
    );
    let is_sorted = sorting.is_sorted;
    let string_comparer = sorting.string_comparer;
    let preferences = UserPreferences {
        organize_imports_type_order: sorting.type_order,
        ..UserPreferences::default()
    };
    let specifier_comparer = Box::new(move |s1: &ast::Node, s2: &ast::Node| {
        compare_import_or_export_specifiers(
            store,
            s1,
            s2,
            |a, b| string_comparer(a, b),
            preferences.clone(),
        )
    });

    (specifier_comparer, is_sorted)
}
