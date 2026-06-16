use ts_ast as ast;
use ts_astnav as astnav;
use ts_core::{Tristate, bool_to_tristate};
use ts_lsproto::FormattingOptions;
use ts_scanner as scanner;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub enum IndentStyle {
    None,
    Block,
    Smart,
}

pub enum FormatSettingValue<'a> {
    String(&'a str),
    Float(f64),
    Int(i32),
}

pub fn parse_indent_style(v: FormatSettingValue<'_>) -> IndentStyle {
    match v {
        FormatSettingValue::String(s) => match s.to_ascii_lowercase().as_str() {
            "none" => IndentStyle::None,
            "block" => IndentStyle::Block,
            "smart" => IndentStyle::Smart,
            _ => IndentStyle::Smart,
        },
        FormatSettingValue::Float(s) => indent_style_from_i32(s as i32),
        FormatSettingValue::Int(s) => indent_style_from_i32(s),
    }
}

fn indent_style_from_i32(v: i32) -> IndentStyle {
    match v {
        0 => IndentStyle::None,
        1 => IndentStyle::Block,
        2 => IndentStyle::Smart,
        _ => IndentStyle::Smart,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub enum SemicolonPreference {
    Ignore,
    Insert,
    Remove,
}

pub fn parse_semicolon_preference(v: FormatSettingValue<'_>) -> SemicolonPreference {
    if let FormatSettingValue::String(s) = v {
        match s.to_ascii_lowercase().as_str() {
            "ignore" => return SemicolonPreference::Ignore,
            "insert" => return SemicolonPreference::Insert,
            "remove" => return SemicolonPreference::Remove,
            _ => {}
        }
    }
    SemicolonPreference::Ignore
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSettings {
    pub base_indent_size: i32,
    pub indent_size: i32,
    pub tab_size: i32,
    pub new_line_character: String,
    pub convert_tabs_to_spaces: Tristate,
    pub indent_style: IndentStyle,
    pub trim_trailing_whitespace: Tristate,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            base_indent_size: 0,
            indent_size: 4,
            tab_size: 4,
            new_line_character: "\n".to_string(),
            convert_tabs_to_spaces: Tristate::True,
            indent_style: IndentStyle::Smart,
            trim_trailing_whitespace: Tristate::True,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatCodeSettings {
    pub editor_settings: EditorSettings,
    pub base_indent_size: i32,
    pub indent_size: i32,
    pub tab_size: i32,
    pub new_line_character: String,
    pub convert_tabs_to_spaces: Tristate,
    pub indent_style: IndentStyle,
    pub trim_trailing_whitespace: Tristate,
    pub insert_space_after_comma_delimiter: Tristate,
    pub insert_space_after_semicolon_in_for_statements: Tristate,
    pub insert_space_before_and_after_binary_operators: Tristate,
    pub insert_space_after_constructor: Tristate,
    pub insert_space_after_keywords_in_control_flow_statements: Tristate,
    pub insert_space_after_function_keyword_for_anonymous_functions: Tristate,
    pub insert_space_after_opening_and_before_closing_nonempty_parenthesis: Tristate,
    pub insert_space_after_opening_and_before_closing_nonempty_brackets: Tristate,
    pub insert_space_after_opening_and_before_closing_nonempty_braces: Tristate,
    pub insert_space_after_opening_and_before_closing_empty_braces: Tristate,
    pub insert_space_after_opening_and_before_closing_template_string_braces: Tristate,
    pub insert_space_after_opening_and_before_closing_jsx_expression_braces: Tristate,
    pub insert_space_after_type_assertion: Tristate,
    pub insert_space_before_function_parenthesis: Tristate,
    pub place_open_brace_on_new_line_for_functions: Tristate,
    pub place_open_brace_on_new_line_for_control_blocks: Tristate,
    pub insert_space_before_type_annotation: Tristate,
    pub indent_multi_line_object_literal_beginning_on_blank_line: Tristate,
    pub semicolons: SemicolonPreference,
    pub indent_switch_case: Tristate,
}

impl Default for FormatCodeSettings {
    fn default() -> Self {
        get_default_format_code_settings()
    }
}

pub fn get_default_format_code_settings() -> FormatCodeSettings {
    let editor_settings = EditorSettings::default();
    FormatCodeSettings {
        base_indent_size: editor_settings.base_indent_size,
        indent_size: editor_settings.indent_size,
        tab_size: editor_settings.tab_size,
        new_line_character: editor_settings.new_line_character.clone(),
        convert_tabs_to_spaces: editor_settings.convert_tabs_to_spaces,
        indent_style: editor_settings.indent_style,
        trim_trailing_whitespace: editor_settings.trim_trailing_whitespace,
        editor_settings,
        insert_space_after_constructor: Tristate::False,
        insert_space_after_comma_delimiter: Tristate::True,
        insert_space_after_semicolon_in_for_statements: Tristate::True,
        insert_space_before_and_after_binary_operators: Tristate::True,
        insert_space_after_keywords_in_control_flow_statements: Tristate::True,
        insert_space_after_function_keyword_for_anonymous_functions: Tristate::False,
        insert_space_after_opening_and_before_closing_nonempty_parenthesis: Tristate::False,
        insert_space_after_opening_and_before_closing_nonempty_brackets: Tristate::False,
        insert_space_after_opening_and_before_closing_nonempty_braces: Tristate::True,
        insert_space_after_opening_and_before_closing_empty_braces: Tristate::Unknown,
        insert_space_after_opening_and_before_closing_template_string_braces: Tristate::False,
        insert_space_after_opening_and_before_closing_jsx_expression_braces: Tristate::False,
        insert_space_after_type_assertion: Tristate::Unknown,
        insert_space_before_function_parenthesis: Tristate::False,
        place_open_brace_on_new_line_for_functions: Tristate::False,
        place_open_brace_on_new_line_for_control_blocks: Tristate::False,
        insert_space_before_type_annotation: Tristate::Unknown,
        indent_multi_line_object_literal_beginning_on_blank_line: Tristate::Unknown,
        semicolons: SemicolonPreference::Ignore,
        indent_switch_case: Tristate::True,
    }
}

pub fn from_lsp_options(
    mut settings: FormatCodeSettings,
    tab_size: u32,
    insert_spaces: bool,
    trim_trailing_whitespace: Option<bool>,
) -> FormatCodeSettings {
    settings.editor_settings.tab_size = tab_size as i32;
    settings.editor_settings.indent_size = tab_size as i32;
    settings.tab_size = tab_size as i32;
    settings.indent_size = tab_size as i32;
    settings.editor_settings.convert_tabs_to_spaces = bool_to_tristate(insert_spaces);
    settings.convert_tabs_to_spaces = settings.editor_settings.convert_tabs_to_spaces;
    if let Some(trim_trailing_whitespace) = trim_trailing_whitespace {
        settings.editor_settings.trim_trailing_whitespace =
            bool_to_tristate(trim_trailing_whitespace);
        settings.trim_trailing_whitespace = settings.editor_settings.trim_trailing_whitespace;
    }
    settings
}

pub fn from_ls_format_options(
    settings: FormatCodeSettings,
    opt: &FormattingOptions,
) -> FormatCodeSettings {
    from_lsp_options(
        settings,
        opt.tab_size,
        opt.insert_spaces,
        opt.trim_trailing_whitespace,
    )
}

impl FormatCodeSettings {
    pub fn to_ls_format_options(&self) -> FormattingOptions {
        FormattingOptions {
            tab_size: self.tab_size as u32,
            insert_spaces: self.convert_tabs_to_spaces.is_true(),
            trim_trailing_whitespace: Some(self.trim_trailing_whitespace.is_true()),
            insert_final_newline: None,
            trim_final_newlines: None,
        }
    }
}

pub fn get_last_child(node: &ast::Node, source_file: &ast::SourceFile) -> Option<ast::Node> {
    get_last_visited_child(node, source_file)
}

fn get_last_child_info(
    node: &ast::Node,
    source_file: &ast::SourceFile,
) -> Option<(ast::Kind, Option<ast::Node>)> {
    let last_child_node = get_last_visited_child(node, source_file);
    let store = source_file.store();

    let token_start_pos =
        last_child_node.map_or_else(|| store.loc(*node).pos(), |node| store.loc(node).end());
    let mut last_token_kind = None;
    let mut scan = scanner::get_scanner_for_source_file(source_file, token_start_pos as usize);
    let mut start_pos = token_start_pos;
    while start_pos < store.loc(*node).end() {
        last_token_kind = Some(scan.token());
        start_pos = scan.token_end();
        scan.scan();
    }

    if let Some(kind) = last_token_kind {
        Some((kind, None))
    } else {
        last_child_node.map(|node| (store.kind(node), Some(node)))
    }
}

pub fn get_last_token(node: Option<ast::Node>, source_file: &ast::SourceFile) -> Option<ast::Node> {
    let node = node?;

    let store = source_file.store();
    if ast::is_token_kind(store.kind(node)) || ast::is_identifier(store, node) {
        return None;
    }

    assert_has_real_position(&node, source_file);

    let last_child = get_last_child(&node, source_file)?;
    if store.kind(last_child) < ast::Kind::FirstNode {
        return Some(last_child);
    }

    get_last_token(Some(last_child), source_file)
}

fn get_last_token_kind(node: &ast::Node, source_file: &ast::SourceFile) -> Option<ast::Kind> {
    let store = source_file.store();
    if ast::is_token_kind(store.kind(*node)) || ast::is_identifier(store, *node) {
        return None;
    }

    assert_has_real_position(node, source_file);

    let (last_child_kind, last_child_node) = get_last_child_info(node, source_file)?;
    if last_child_kind < ast::Kind::FirstNode {
        return Some(last_child_kind);
    }

    last_child_node.and_then(|node| get_last_token_kind(&node, source_file))
}

pub fn get_last_visited_child(
    node: &ast::Node,
    source_file: &ast::SourceFile,
) -> Option<ast::Node> {
    let mut last_child: Option<ast::Node> = None;
    let store = source_file.store();
    let _ = store.for_each_present_child(*node, |n| {
        if !store.flags(n).contains(ast::NodeFlags::REPARSED) {
            last_child = Some(n);
        }
        std::ops::ControlFlow::Continue(())
    });
    last_child
}

pub fn get_first_token(
    node: Option<ast::Node>,
    source_file: &ast::SourceFile,
) -> Option<ast::Node> {
    let node = node?;
    let store = source_file.store();
    if ast::is_identifier(store, node) || ast::is_token_kind(store.kind(node)) {
        return None;
    }

    assert_has_real_position(&node, source_file);

    let mut first_child: Option<ast::Node> = None;
    let _ = store.for_each_present_child(node, |n| {
        if store.flags(n).contains(ast::NodeFlags::REPARSED) {
            return std::ops::ControlFlow::Continue(());
        }
        first_child = Some(n);
        std::ops::ControlFlow::Break(())
    });
    if first_child.is_none() {
        return None;
    }

    if first_child.is_some_and(|child| store.kind(child) < ast::Kind::FirstNode) {
        return first_child;
    }

    get_first_token(first_child, source_file)
}

pub fn assert_has_real_position(node: &ast::Node, source_file: &ast::SourceFile) {
    let loc = source_file.store().loc(*node);
    if ast::position_is_synthesized(loc.pos()) || ast::position_is_synthesized(loc.end()) {
        panic!("Node must have a real position for this operation.");
    }
}

pub fn position_belongs_to_node(
    candidate: &ast::Node,
    position: i32,
    file: &ast::SourceFile,
) -> bool {
    let candidate_loc = file.store().loc(*candidate);
    if candidate_loc.pos() > position {
        panic!("Expected candidate.pos <= position");
    }
    position < candidate_loc.end() || !is_completed_node(Some(*candidate), file)
}

pub fn is_completed_node(n: Option<ast::Node>, source_file: &ast::SourceFile) -> bool {
    let Some(n) = n else {
        return false;
    };
    let store = source_file.store();
    if ast::node_is_missing(store, Some(n)) {
        return false;
    }

    match store.kind(n) {
        ast::Kind::ClassDeclaration
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::EnumDeclaration
        | ast::Kind::ObjectLiteralExpression
        | ast::Kind::ObjectBindingPattern
        | ast::Kind::TypeLiteral
        | ast::Kind::Block
        | ast::Kind::ModuleBlock
        | ast::Kind::CaseBlock
        | ast::Kind::NamedImports
        | ast::Kind::NamedExports => node_ends_with(&n, ast::Kind::CloseBraceToken, source_file),

        ast::Kind::CatchClause => is_completed_node(store.block(n), source_file),

        ast::Kind::NewExpression => {
            if store.arguments(n).is_none() {
                true
            } else {
                node_ends_with(&n, ast::Kind::CloseParenToken, source_file)
            }
        }

        ast::Kind::CallExpression
        | ast::Kind::ParenthesizedExpression
        | ast::Kind::ParenthesizedType => {
            node_ends_with(&n, ast::Kind::CloseParenToken, source_file)
        }

        ast::Kind::FunctionType | ast::Kind::ConstructorType => {
            is_completed_node(store.r#type(n), source_file)
        }

        ast::Kind::Constructor
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor
        | ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression
        | ast::Kind::MethodDeclaration
        | ast::Kind::MethodSignature
        | ast::Kind::ConstructSignature
        | ast::Kind::CallSignature
        | ast::Kind::ArrowFunction => {
            if let Some(body) = store.body(n) {
                return is_completed_node(Some(body), source_file);
            }
            if let Some(node_type) = store.r#type(n) {
                return is_completed_node(Some(node_type), source_file);
            }
            has_child_of_kind(&n, ast::Kind::CloseParenToken, source_file)
        }

        ast::Kind::ModuleDeclaration => store
            .body(n)
            .is_some_and(|body| is_completed_node(Some(body), source_file)),

        ast::Kind::IfStatement => {
            if let Some(else_statement) = store.else_statement(n) {
                return is_completed_node(Some(else_statement), source_file);
            }
            is_completed_node(store.then_statement(n), source_file)
        }

        ast::Kind::ExpressionStatement => {
            is_completed_node(store.expression(n), source_file)
                || has_child_of_kind(&n, ast::Kind::SemicolonToken, source_file)
        }

        ast::Kind::ArrayLiteralExpression
        | ast::Kind::ArrayBindingPattern
        | ast::Kind::ElementAccessExpression
        | ast::Kind::ComputedPropertyName
        | ast::Kind::TupleType => node_ends_with(&n, ast::Kind::CloseBracketToken, source_file),

        ast::Kind::IndexSignature => {
            if let Some(node_type) = store.r#type(n) {
                return is_completed_node(Some(node_type), source_file);
            }
            has_child_of_kind(&n, ast::Kind::CloseBracketToken, source_file)
        }

        ast::Kind::CaseClause | ast::Kind::DefaultClause => false,

        ast::Kind::ForStatement
        | ast::Kind::ForInStatement
        | ast::Kind::ForOfStatement
        | ast::Kind::WhileStatement => is_completed_node(store.statement(n), source_file),

        ast::Kind::DoStatement => {
            if has_child_of_kind(&n, ast::Kind::WhileKeyword, source_file) {
                return node_ends_with(&n, ast::Kind::CloseParenToken, source_file);
            }
            is_completed_node(store.statement(n), source_file)
        }

        ast::Kind::TypeQuery => is_completed_node(store.expr_name(n), source_file),

        ast::Kind::TypeOfExpression
        | ast::Kind::DeleteExpression
        | ast::Kind::VoidExpression
        | ast::Kind::YieldExpression
        | ast::Kind::SpreadElement => is_completed_node(store.expression(n), source_file),

        ast::Kind::TaggedTemplateExpression => is_completed_node(store.template(n), source_file),

        ast::Kind::TemplateExpression => {
            let Some(spans) = store.template_spans(n) else {
                return false;
            };
            !spans.is_empty()
                && spans
                    .iter()
                    .last()
                    .is_some_and(|span| is_completed_node(Some(span), source_file))
        }

        ast::Kind::TemplateSpan => node_is_present(store, store.literal(n)),

        ast::Kind::ExportDeclaration | ast::Kind::ImportDeclaration => {
            node_is_present(store, store.module_specifier(n))
        }

        ast::Kind::PrefixUnaryExpression => is_completed_node(store.operand(n), source_file),

        ast::Kind::BinaryExpression => is_completed_node(store.right(n), source_file),

        ast::Kind::ConditionalExpression => is_completed_node(store.when_false(n), source_file),

        _ => true,
    }
}

fn node_is_present(store: &ast::AstStore, node: Option<ast::Node>) -> bool {
    match node {
        Some(node) => ast::node_is_present(store, Some(node)),
        None => ast::node_is_present(store, None),
    }
}

fn node_ends_with<'a>(
    n: &'a ast::Node,
    expected_last_token: ast::Kind,
    source_file: &'a ast::SourceFile,
) -> bool {
    let store = source_file.store();
    let mut last_kind = None;
    let mut previous_kind = None;
    let token_start_pos = if let Some(last_child) = get_last_visited_child(n, source_file) {
        last_kind = Some(store.kind(last_child));
        store.loc(last_child).end()
    } else {
        store.loc(*n).pos()
    };

    let mut scan = scanner::get_scanner_for_source_file(source_file, token_start_pos as usize);
    let mut start_pos = token_start_pos;
    while start_pos < store.loc(*n).end() {
        previous_kind = last_kind;
        last_kind = Some(scan.token());
        let token_end = scan.token_end();
        start_pos = token_end;
        scan.scan();
    }

    if last_kind == Some(expected_last_token) {
        true
    } else if last_kind == Some(ast::Kind::SemicolonToken) {
        previous_kind == Some(expected_last_token)
    } else {
        false
    }
}

fn has_child_of_kind(
    containing_node: &ast::Node,
    kind: ast::Kind,
    source_file: &ast::SourceFile,
) -> bool {
    astnav::find_child_of_kind(*containing_node, kind, source_file).is_some()
}

pub fn position_is_asi_candidate(pos: i32, context: &ast::Node, file: &ast::SourceFile) -> bool {
    let store = file.store();
    let mut current = Some(context.clone());
    let mut context_ancestor = None;
    while let Some(ancestor) = current {
        if store.loc(ancestor).end() != pos {
            break;
        }
        if syntax_may_be_asi_candidate(store.kind(ancestor)) {
            context_ancestor = Some(ancestor);
            break;
        }
        current = store.parent(ancestor);
    }

    context_ancestor.is_some_and(|ancestor| node_is_asi_candidate(&ancestor, file))
}

pub fn syntax_may_be_asi_candidate(kind: ast::Kind) -> bool {
    syntax_requires_trailing_comma_or_semicolon_or_asi(kind)
        || syntax_requires_trailing_function_block_or_semicolon_or_asi(kind)
        || syntax_requires_trailing_module_block_or_semicolon_or_asi(kind)
        || syntax_requires_trailing_semicolon_or_asi(kind)
}

pub fn syntax_requires_trailing_comma_or_semicolon_or_asi(kind: ast::Kind) -> bool {
    kind == ast::Kind::CallSignature
        || kind == ast::Kind::ConstructSignature
        || kind == ast::Kind::IndexSignature
        || kind == ast::Kind::PropertySignature
        || kind == ast::Kind::MethodSignature
}

pub fn syntax_requires_trailing_function_block_or_semicolon_or_asi(kind: ast::Kind) -> bool {
    kind == ast::Kind::FunctionDeclaration
        || kind == ast::Kind::Constructor
        || kind == ast::Kind::MethodDeclaration
        || kind == ast::Kind::GetAccessor
        || kind == ast::Kind::SetAccessor
}

pub fn syntax_requires_trailing_module_block_or_semicolon_or_asi(kind: ast::Kind) -> bool {
    kind == ast::Kind::ModuleDeclaration
}

pub fn syntax_requires_trailing_semicolon_or_asi(kind: ast::Kind) -> bool {
    kind == ast::Kind::VariableStatement
        || kind == ast::Kind::ExpressionStatement
        || kind == ast::Kind::DoStatement
        || kind == ast::Kind::ContinueStatement
        || kind == ast::Kind::BreakStatement
        || kind == ast::Kind::ReturnStatement
        || kind == ast::Kind::ThrowStatement
        || kind == ast::Kind::DebuggerStatement
        || kind == ast::Kind::PropertyDeclaration
        || kind == ast::Kind::TypeAliasDeclaration
        || kind == ast::Kind::ImportDeclaration
        || kind == ast::Kind::ImportEqualsDeclaration
        || kind == ast::Kind::ExportDeclaration
        || kind == ast::Kind::NamespaceExportDeclaration
        || kind == ast::Kind::ExportAssignment
}

pub fn node_is_asi_candidate(node: &ast::Node, file: &ast::SourceFile) -> bool {
    let store = file.store();
    let last_token_kind = get_last_token_kind(node, file);
    if last_token_kind == Some(ast::Kind::SemicolonToken) {
        return false;
    }

    let node_kind = store.kind(*node);
    if syntax_requires_trailing_comma_or_semicolon_or_asi(node_kind) {
        if last_token_kind == Some(ast::Kind::CommaToken) {
            return false;
        }
    } else if syntax_requires_trailing_module_block_or_semicolon_or_asi(node_kind) {
        let last_child = get_last_child_info(node, file);
        if last_child
            .is_some_and(|(_, node)| node.is_some_and(|node| ast::is_module_block(store, node)))
        {
            return false;
        }
    } else if syntax_requires_trailing_function_block_or_semicolon_or_asi(node_kind) {
        let last_child = get_last_child_info(node, file);
        if last_child.is_some_and(|(_, node)| {
            node.is_some_and(|node| ast::is_function_block(store, Some(node)))
        }) {
            return false;
        }
    } else if !syntax_requires_trailing_semicolon_or_asi(node_kind) {
        return false;
    }

    if node_kind == ast::Kind::DoStatement {
        return true;
    }

    let mut top_node = node.clone();
    while let Some(parent) = store.parent(top_node) {
        top_node = parent;
    }

    let next_token = astnav::find_next_token(*node, top_node, file);
    if next_token.is_none()
        || next_token.is_some_and(|token| store.kind(token) == ast::Kind::CloseBraceToken)
    {
        return true;
    }

    let start_line = scanner::get_ecma_line_of_position(file, store.loc(*node).end());
    let end_line = scanner::get_ecma_line_of_position(
        file,
        astnav::get_start_of_node(next_token.unwrap(), file),
    );
    start_line != end_line
}
