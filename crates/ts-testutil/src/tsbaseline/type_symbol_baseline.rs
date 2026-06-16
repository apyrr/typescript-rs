use std::cell::RefCell;
use std::collections::HashMap;

use crate::baseline;
use crate::harnessutil::{ProgramHandle, TestFile};

use super::{is_default_library_file, remove_test_path_prefixes};

use ts_ast as ast;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_scanner as scanner;
use ts_tspath as tspath;

pub fn do_type_and_symbol_baseline(
    baseline_path: &str,
    header: &str,
    program: Option<ProgramHandle>,
    all_files: &[TestFile],
    opts: baseline::Options,
    skip_type_baselines: bool,
    skip_symbol_baselines: bool,
    has_error_baseline: bool,
) -> Result<(), String> {
    let walker = TypeWriterWalker::new(program, has_error_baseline);
    if !skip_type_baselines {
        let mut types_opts = opts.clone();
        types_opts.diff_fixup_old = Some(fixup_old_type_baseline);
        check_baselines(baseline_path, all_files, &walker, header, types_opts, false)?;
    }
    if !skip_symbol_baselines {
        check_baselines(baseline_path, all_files, &walker, header, opts, true)?;
    }
    Ok(())
}

pub fn is_type_baseline_node_reuse_line(line: &str) -> bool {
    let Some(line) = line.strip_prefix('>') else {
        return false;
    };
    let line = line.get(1..).unwrap_or_default().trim_start();
    let Some(line) = line.strip_prefix(':') else {
        return false;
    };
    line.chars().all(|ch| matches!(ch, ' ' | '^' | '\r'))
}

pub fn check_baselines(
    baseline_path: &str,
    all_files: &[TestFile],
    full_walker: &TypeWriterWalker,
    header: &str,
    opts: baseline::Options,
    is_symbol_baseline: bool,
) -> Result<(), String> {
    let extension = if is_symbol_baseline {
        ".symbols"
    } else {
        ".types"
    };
    let output_file_name = replace_ts_extension(baseline_path, extension);
    let full_baseline = generate_baseline(all_files, full_walker, header, is_symbol_baseline);
    baseline::run(&output_file_name, &full_baseline, opts)
}

pub fn generate_baseline(
    all_files: &[TestFile],
    full_walker: &TypeWriterWalker,
    header: &str,
    is_symbol_baseline: bool,
) -> String {
    let baselines = iterate_baseline(all_files, full_walker, is_symbol_baseline);
    if baselines.is_empty() {
        baseline::NO_CONTENT.to_string()
    } else {
        format!("//// [{header}] ////\r\n\r\n{}", baselines.join(""))
    }
}

pub fn iterate_baseline(
    all_files: &[TestFile],
    full_walker: &TypeWriterWalker,
    is_symbol_baseline: bool,
) -> Vec<String> {
    let mut baselines = Vec::new();

    for file in all_files {
        let results = if is_symbol_baseline {
            full_walker.get_symbols(&file.unit_name)
        } else {
            full_walker.get_types(&file.unit_name)
        };

        if is_symbol_baseline && results.iter().any(|result| result.symbol.is_empty()) {
            return baselines;
        }

        let mut out = format!("=== {} ===\r\n", file.unit_name);
        let code_lines = split_code_lines(&file.content);
        let mut last_index_written: Option<usize> = None;

        for result in results {
            write_source_lines_before_result(
                &mut out,
                &code_lines,
                last_index_written,
                result.line,
            );
            last_index_written = Some(result.line);

            let type_or_symbol = if is_symbol_baseline {
                &result.symbol
            } else {
                &result.typ
            };
            let line_text = line_end_stripped(&result.source_text);
            out.push_str(&format!(">{line_text} : {type_or_symbol}\r\n"));
            if !result.underline.is_empty() {
                out.push('>');
                out.push_str(&" ".repeat(line_text.len()));
                out.push_str(" : ");
                out.push_str(&result.underline);
                out.push_str("\r\n");
            }
        }

        write_remaining_source_lines(&mut out, &code_lines, last_index_written);
        out.push_str("\r\n");
        baselines.push(remove_test_path_prefixes(&out, false));
    }

    baselines
}

pub fn fixup_old_type_baseline(text: String) -> String {
    let mut out = String::with_capacity(text.len());
    let mut perf_stats = false;

    for mut line in text.split('\n') {
        if is_type_baseline_node_reuse_line(line) {
            continue;
        }

        if !perf_stats && line.starts_with("=== Performance Stats ===") {
            perf_stats = true;
            continue;
        }
        if perf_stats {
            if line.starts_with("=== ") {
                perf_stats = false;
            } else {
                continue;
            }
        }

        const RELATIVE_PREFIX_NEW: &str = "=== ";
        const RELATIVE_PREFIX_OLD: &str = "=== ./";
        if let Some(rest) = line.strip_prefix(RELATIVE_PREFIX_OLD) {
            out.push_str(RELATIVE_PREFIX_NEW);
            line = rest;
        }

        out.push_str(line);
        out.push('\n');
    }

    out.pop();
    out
}

pub struct TypeWriterWalker {
    pub program: Option<ProgramHandle>,
    pub had_error_baseline: bool,
    declaration_text_cache: RefCell<HashMap<NodeKey, String>>,
}

impl TypeWriterWalker {
    pub fn new(program: Option<ProgramHandle>, had_error_baseline: bool) -> Self {
        Self {
            program,
            had_error_baseline,
            declaration_text_cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn get_types(&self, filename: &str) -> Vec<TypeWriterResult> {
        let Some(program) = self.program.as_ref() else {
            return Vec::new();
        };
        let file_name = tspath::normalize_path(filename);
        let Some(source_file) = program.0.get_source_file_ref(&file_name) else {
            return Vec::new();
        };
        let ctx = core::Context::background();
        program.0.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(&ctx),
            source_file,
            |file_checker| {
                self.visit_node(source_file, &source_file.as_node(), false, file_checker)
            },
        )
    }

    pub fn get_symbols(&self, filename: &str) -> Vec<TypeWriterResult> {
        let Some(program) = self.program.as_ref() else {
            return Vec::new();
        };
        let file_name = tspath::normalize_path(filename);
        let Some(source_file) = program.0.get_source_file_ref(&file_name) else {
            return Vec::new();
        };
        let ctx = core::Context::background();
        program.0.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(&ctx),
            source_file,
            |file_checker| self.visit_node(source_file, &source_file.as_node(), true, file_checker),
        )
    }

    pub fn visit_node(
        &self,
        source_file: &ast::SourceFile,
        node: &ast::Node,
        is_symbol_walk: bool,
        file_checker: &mut checker::Checker<'_, '_>,
    ) -> Vec<TypeWriterResult> {
        let mut results = Vec::new();
        let store = source_file.store();
        for node in for_each_ast_node(store, *node) {
            if ast::is_expression_node(store, &node)
                || store.kind(node) == ast::Kind::Identifier
                || ast::is_declaration_name(store, &node)
            {
                if let Some(result) =
                    self.write_type_or_symbol(source_file, &node, is_symbol_walk, file_checker)
                {
                    results.push(result);
                }
            }
        }
        results
    }

    pub fn visit_nodes(
        &self,
        source_file: &ast::SourceFile,
        nodes: &[ast::Node],
        is_symbol_walk: bool,
        file_checker: &mut checker::Checker<'_, '_>,
    ) -> Vec<TypeWriterResult> {
        let mut results = Vec::new();
        for node in nodes {
            results.extend(self.visit_node(source_file, node, is_symbol_walk, file_checker));
        }
        results
    }

    pub fn write_type_or_symbol(
        &self,
        source_file: &ast::SourceFile,
        node: &ast::Node,
        is_symbol_walk: bool,
        file_checker: &mut checker::Checker<'_, '_>,
    ) -> Option<TypeWriterResult> {
        let store = source_file.store();
        let actual_pos =
            scanner::skip_trivia(source_file.text(), store.loc(*node).pos().max(0) as usize);
        let line = scanner::get_ecma_line_of_position(source_file, actual_pos);
        let source_text =
            scanner::get_source_text_of_node_from_source_file(source_file, node, false);

        if !is_symbol_walk {
            if self.should_skip_type_node(source_file, node) {
                return None;
            }

            let mut typ = None;
            let parent = store.parent(*node);
            if parent.as_ref().is_some_and(|parent| {
                ast::is_expression_with_type_arguments_in_class_extends_clause(store, parent)
            }) {
                typ = parent.map(|parent| file_checker.get_type_at_location(parent));
            }
            if typ.is_none() || checker::is_type_any(file_checker, typ) {
                typ = Some(file_checker.get_type_at_location(*node));
            }
            let typ = typ?;

            let type_string = if !self.had_error_baseline
                && checker::is_type_any(file_checker, Some(typ))
                && !store
                    .parent(*node)
                    .as_ref()
                    .is_some_and(|parent| ast::is_binding_element(store, *parent))
                && !store
                    .parent(*node)
                    .as_ref()
                    .is_some_and(|parent| ast::is_property_access_or_qualified_name(store, *parent))
                && !ast::is_label_name(store, node)
                && !store
                    .parent(*node)
                    .as_ref()
                    .is_some_and(|parent| ast::is_global_scope_augmentation(store, *parent))
                && !store
                    .parent(*node)
                    .as_ref()
                    .is_some_and(|parent| ast::is_meta_property(store, *parent))
                && !is_import_statement_name(store, *node)
                && !is_export_statement_name(store, *node)
                && !is_intrinsic_jsx_tag(store, *node, &source_file)
            {
                file_checker
                    .intrinsic_type_name_public(typ)
                    .unwrap_or_default()
            } else {
                file_checker.type_to_baseline_string_public(typ, *node)
            };
            return Some(TypeWriterResult {
                line,
                source_text,
                typ: type_string,
                ..TypeWriterResult::default()
            });
        }

        let symbol = file_checker.get_symbol_at_location_public(*node)?;
        let declarations = file_checker.collect_symbol_declarations_public(symbol);
        let mut symbol_string = String::with_capacity(256);
        symbol_string.push_str("Symbol(");
        symbol_string.push_str(&ast::escape_all_internal_symbol_names(
            &file_checker
                .symbol_identity_to_string_ex_public(
                    symbol,
                    source_file.store().parent(*node),
                    ast::SYMBOL_FLAGS_NONE,
                    checker::SYMBOL_FORMAT_FLAGS_ALLOW_ANY_NODE_KIND,
                )
                .unwrap_or_default(),
        ));
        for (count, declaration) in declarations.iter().enumerate() {
            if count >= 5 {
                symbol_string.push_str(&format!(" ... and {} more", declarations.len() - count));
                break;
            }
            symbol_string.push_str(", ");
            let key = NodeKey::new(*declaration);
            if let Some(cached) = self.declaration_text_cache.borrow().get(&key) {
                symbol_string.push_str(cached);
                continue;
            }
            let decl_text = self.declaration_text(declaration);
            self.declaration_text_cache
                .borrow_mut()
                .insert(key, decl_text.clone());
            symbol_string.push_str(&decl_text);
        }
        symbol_string.push(')');
        Some(TypeWriterResult {
            line,
            source_text,
            symbol: symbol_string,
            ..TypeWriterResult::default()
        })
    }

    fn should_skip_type_node(&self, source_file: &ast::SourceFile, node: &ast::Node) -> bool {
        let store = source_file.store();
        if ast::is_part_of_type_node(store, node) {
            return true;
        }
        if matches!(
            store.kind(*node),
            ast::Kind::AsExpression | ast::Kind::SatisfiesExpression
        ) && store
            .r#type(*node)
            .is_some_and(|typ| store.flags(typ).contains(ast::NodeFlags::REPARSED))
        {
            return true;
        }
        if ast::is_identifier(store, *node) {
            if let Some(parent) = store.parent(*node) {
                let has_value_meaning = (get_meaning_from_declaration(store, &parent).0
                    & ast::SemanticMeaning::VALUE.0)
                    != 0;
                let is_alias_name = ast::is_type_or_js_type_alias_declaration(store, &parent)
                    && store
                        .name(parent)
                        .is_some_and(|name| node_same(name, *node));
                if !has_value_meaning && !is_alias_name {
                    return true;
                }
            }
        }
        ast::is_omitted_expression(store, *node)
    }

    fn source_file_for_node(&self, node: &ast::Node) -> Option<ast::SourceFile> {
        self.program
            .as_ref()?
            .0
            .get_source_files()
            .into_iter()
            .find(|file| file.store().store_id() == node.store_id())
    }

    fn declaration_text(&self, declaration: &ast::Node) -> String {
        let Some(decl_source_file) = self.source_file_for_node(declaration) else {
            return "Decl(unknown, --, --)".to_string();
        };
        let (decl_line, decl_char) = scanner::get_ecma_line_and_utf16_character_of_position(
            &decl_source_file,
            decl_source_file.store().loc(*declaration).pos(),
        );
        let file_name = tspath::get_base_file_name(&decl_source_file.file_name());
        if is_default_library_file(&file_name) {
            format!("Decl({file_name}, --, --)")
        } else {
            format!("Decl({file_name}, {decl_line}, {decl_char})")
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypeWriterResult {
    pub line: usize,
    pub column: usize,
    pub source_text: String,
    pub typ: String,
    pub symbol: String,
    pub underline: String,
}

fn split_code_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, ch) in text.char_indices() {
        if matches!(ch, '\r' | '\u{2028}' | '\u{2029}' | '\n') {
            lines.push(&text[start..index]);
            start = index + ch.len_utf8();
        }
    }
    lines.push(&text[start..]);
    lines
}

fn write_source_lines_before_result(
    out: &mut String,
    code_lines: &[&str],
    last_index_written: Option<usize>,
    result_line: usize,
) {
    match last_index_written {
        None => {
            out.push_str(
                &code_lines[..=result_line.min(code_lines.len().saturating_sub(1))].join("\r\n"),
            );
            out.push_str("\r\n");
        }
        Some(last) if last != result_line => {
            if should_separate_from_next_source_line(code_lines, last) {
                out.push_str("\r\n");
            }
            let start = (last + 1).min(code_lines.len());
            let end = result_line.min(code_lines.len().saturating_sub(1));
            if start <= end {
                out.push_str(&code_lines[start..=end].join("\r\n"));
                out.push_str("\r\n");
            }
        }
        _ => {}
    }
}

fn write_remaining_source_lines(
    out: &mut String,
    code_lines: &[&str],
    last_index_written: Option<usize>,
) {
    let start = last_index_written.map_or(0, |line| line + 1);
    if start >= code_lines.len() {
        return;
    }
    let should_separate = match last_index_written {
        Some(last) => should_separate_from_next_source_line(code_lines, last),
        None => code_lines
            .first()
            .is_some_and(|line| !is_bracket_line(line) && !line.trim().is_empty()),
    };
    if should_separate {
        out.push_str("\r\n");
    }
    out.push_str(&code_lines[start..].join("\r\n"));
}

fn should_separate_from_next_source_line(code_lines: &[&str], last_index_written: usize) -> bool {
    let Some(next_line) = code_lines.get(last_index_written + 1) else {
        return false;
    };
    !is_bracket_line(next_line) && !next_line.trim().is_empty()
}

fn is_bracket_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed == "{" || trimmed == "}" || trimmed == "|"
}

fn line_end_stripped(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' && chars.peek() == Some(&'\n') {
            chars.next();
            continue;
        }
        if ch == '\n' {
            continue;
        }
        result.push(ch);
    }
    result
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct NodeKey {
    node: ast::Node,
}

impl NodeKey {
    fn new(node: ast::Node) -> Self {
        Self { node }
    }
}

pub fn for_each_ast_node(store: &ast::AstStore, node: ast::Node) -> Vec<ast::Node> {
    let mut result = Vec::new();
    let mut work = vec![canonical_node(store, node)];

    while let Some(elem) = work.pop() {
        let elem = canonical_node(store, elem);
        let parent = store.parent(elem);
        let is_reparsed = store.flags(elem).contains(ast::NodeFlags::REPARSED);
        let is_as_or_satisfies = matches!(
            store.kind(elem),
            ast::Kind::AsExpression | ast::Kind::SatisfiesExpression
        );
        let parent_is_as_or_satisfies = parent.as_ref().is_some_and(|parent| {
            matches!(
                store.kind(*parent),
                ast::Kind::AsExpression | ast::Kind::SatisfiesExpression
            )
        });
        let is_parent_expression = parent_is_as_or_satisfies
            && parent
                .as_ref()
                .and_then(|parent| store.expression(*parent))
                .is_some_and(|expression| node_same(expression, elem));

        if !is_reparsed || is_as_or_satisfies || (parent_is_as_or_satisfies && is_parent_expression)
        {
            if !is_reparsed || parent_is_as_or_satisfies {
                result.push(elem);
            }
            let mut children = Vec::new();
            let _ = store.for_each_present_child(elem, |child| {
                children.push(canonical_node(store, child));
                std::ops::ControlFlow::Continue(())
            });
            children.reverse();
            work.extend(children);
        }
    }
    result
}

fn canonical_node(store: &ast::AstStore, node: ast::Node) -> ast::Node {
    ast::get_reparsed_node_for_node(store, &node)
}

pub fn is_import_statement_name(store: &ast::AstStore, node: ast::Node) -> bool {
    let Some(parent) = store.parent(node) else {
        return false;
    };
    if ast::is_import_specifier(store, parent)
        && (store.name(parent).is_some_and(|name| node_same(name, node))
            || store
                .property_name(parent)
                .is_some_and(|property_name| node_same(property_name, node)))
    {
        return true;
    }
    if store.kind(parent) == ast::Kind::ImportClause
        && store.name(parent).is_some_and(|name| node_same(name, node))
    {
        return true;
    }
    if store.kind(parent) == ast::Kind::ImportEqualsDeclaration
        && store.name(parent).is_some_and(|name| node_same(name, node))
    {
        return true;
    }
    false
}

pub fn is_export_statement_name(store: &ast::AstStore, node: ast::Node) -> bool {
    let Some(parent) = store.parent(node) else {
        return false;
    };
    if store.kind(parent) == ast::Kind::ExportAssignment
        && store
            .expression(parent)
            .is_some_and(|expression| node_same(expression, node))
    {
        return true;
    }
    if ast::is_export_specifier(store, parent)
        && (store.name(parent).is_some_and(|name| node_same(name, node))
            || store
                .property_name(parent)
                .is_some_and(|property_name| node_same(property_name, node)))
    {
        return true;
    }
    false
}

pub fn is_intrinsic_jsx_tag(
    store: &ast::AstStore,
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> bool {
    let Some(parent) = store.parent(node) else {
        return false;
    };
    if !matches!(
        store.kind(parent),
        ast::Kind::JsxOpeningElement
            | ast::Kind::JsxClosingElement
            | ast::Kind::JsxSelfClosingElement
    ) {
        return false;
    }
    if !store
        .tag_name(parent)
        .is_some_and(|tag_name| node_same(tag_name, node))
    {
        return false;
    }
    let text = scanner::get_source_text_of_node_from_source_file(source_file, &node, false);
    scanner::is_intrinsic_jsx_name(&text)
}

fn node_same(left: ast::Node, right: ast::Node) -> bool {
    left == right
}

fn get_meaning_from_declaration(store: &ast::AstStore, node: &ast::Node) -> ast::SemanticMeaning {
    match store.kind(*node) {
        ast::Kind::VariableDeclaration
        | ast::Kind::Parameter
        | ast::Kind::BindingElement
        | ast::Kind::PropertyDeclaration
        | ast::Kind::PropertySignature
        | ast::Kind::PropertyAssignment
        | ast::Kind::ShorthandPropertyAssignment
        | ast::Kind::MethodDeclaration
        | ast::Kind::MethodSignature
        | ast::Kind::Constructor
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor
        | ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression
        | ast::Kind::ArrowFunction
        | ast::Kind::CatchClause
        | ast::Kind::JsxAttribute => ast::SemanticMeaning::VALUE,
        ast::Kind::TypeParameter
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::TypeAliasDeclaration
        | ast::Kind::JSTypeAliasDeclaration
        | ast::Kind::TypeLiteral => ast::SemanticMeaning::TYPE,
        ast::Kind::EnumMember | ast::Kind::ClassDeclaration => {
            ast::SemanticMeaning(ast::SemanticMeaning::VALUE.0 | ast::SemanticMeaning::TYPE.0)
        }
        ast::Kind::ModuleDeclaration => {
            if ast::is_ambient_module(store, *node)
                || ast::get_module_instance_state(store, *node)
                    == ast::ModuleInstanceState::Instantiated
            {
                ast::SemanticMeaning(
                    ast::SemanticMeaning::NAMESPACE.0 | ast::SemanticMeaning::VALUE.0,
                )
            } else {
                ast::SemanticMeaning::NAMESPACE
            }
        }
        ast::Kind::EnumDeclaration
        | ast::Kind::NamedImports
        | ast::Kind::ImportSpecifier
        | ast::Kind::ImportEqualsDeclaration
        | ast::Kind::ImportDeclaration
        | ast::Kind::JSImportDeclaration
        | ast::Kind::ExportAssignment
        | ast::Kind::ExportDeclaration => ast::SEMANTIC_MEANING_ALL,
        ast::Kind::SourceFile => {
            ast::SemanticMeaning(ast::SemanticMeaning::NAMESPACE.0 | ast::SemanticMeaning::VALUE.0)
        }
        _ => ast::SEMANTIC_MEANING_ALL,
    }
}

fn replace_ts_extension(path: &str, replacement: &str) -> String {
    for ext in [".tsx", ".ts"] {
        if let Some(prefix) = path.strip_suffix(ext) {
            return format!("{prefix}{replacement}");
        }
    }
    format!("{path}{replacement}")
}
