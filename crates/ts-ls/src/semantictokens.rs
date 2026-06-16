use ts_ast as ast;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_modulespecifiers::CheckerShape;
use ts_scanner as scanner;

use crate::LanguageService;
use crate::hover::get_meaning_from_location;
use crate::lsconv;

// tokenTypes defines the order of token types for encoding
pub const TOKEN_TYPES: &[lsproto::SemanticTokenType] = &[
    lsproto::SemanticTokenType::NAMESPACE,
    lsproto::SemanticTokenType::CLASS,
    lsproto::SemanticTokenType::ENUM,
    lsproto::SemanticTokenType::INTERFACE,
    lsproto::SemanticTokenType::STRUCT,
    lsproto::SemanticTokenType::TYPE_PARAMETER,
    lsproto::SemanticTokenType::TYPE,
    lsproto::SemanticTokenType::PARAMETER,
    lsproto::SemanticTokenType::VARIABLE,
    lsproto::SemanticTokenType::PROPERTY,
    lsproto::SemanticTokenType::ENUM_MEMBER,
    lsproto::SemanticTokenType::DECORATOR,
    lsproto::SemanticTokenType::EVENT,
    lsproto::SemanticTokenType::FUNCTION,
    lsproto::SemanticTokenType::METHOD,
    lsproto::SemanticTokenType::MACRO,
    lsproto::SemanticTokenType::new("label"),
    lsproto::SemanticTokenType::COMMENT,
    lsproto::SemanticTokenType::STRING,
    lsproto::SemanticTokenType::KEYWORD,
    lsproto::SemanticTokenType::NUMBER,
    lsproto::SemanticTokenType::REGEXP,
    lsproto::SemanticTokenType::OPERATOR,
];

// tokenModifiers defines the order of token modifiers for encoding
pub const TOKEN_MODIFIERS: &[lsproto::SemanticTokenModifier] = &[
    lsproto::SemanticTokenModifier::DECLARATION,
    lsproto::SemanticTokenModifier::DEFINITION,
    lsproto::SemanticTokenModifier::READONLY,
    lsproto::SemanticTokenModifier::STATIC,
    lsproto::SemanticTokenModifier::DEPRECATED,
    lsproto::SemanticTokenModifier::ABSTRACT,
    lsproto::SemanticTokenModifier::ASYNC,
    lsproto::SemanticTokenModifier::MODIFICATION,
    lsproto::SemanticTokenModifier::DOCUMENTATION,
    lsproto::SemanticTokenModifier::DEFAULT_LIBRARY,
    lsproto::SemanticTokenModifier::new("local"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TokenType {
    Namespace = 0,
    Class,
    Enum,
    Interface,
    Struct,
    TypeParameter,
    Type,
    Parameter,
    Variable,
    Property,
    EnumMember,
    Decorator,
    Event,
    Function,
    Method,
    Macro,
    Label,
    Comment,
    String,
    Keyword,
    Number,
    Regexp,
    Operator,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TokenModifier(u32);

impl TokenModifier {
    const DECLARATION: Self = Self(1 << 0);
    const DEFINITION: Self = Self(1 << 1);
    const READONLY: Self = Self(1 << 2);
    const STATIC: Self = Self(1 << 3);
    const DEPRECATED: Self = Self(1 << 4);
    const ABSTRACT: Self = Self(1 << 5);
    const ASYNC: Self = Self(1 << 6);
    const MODIFICATION: Self = Self(1 << 7);
    const DOCUMENTATION: Self = Self(1 << 8);
    const DEFAULT_LIBRARY: Self = Self(1 << 9);
    const LOCAL: Self = Self(1 << 10);

    fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl std::ops::BitOrAssign for TokenModifier {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Clone, Copy)]
struct SemanticToken {
    node: ast::Node,
    token_type: TokenType,
    token_modifier: TokenModifier,
}

fn source_file_for_node<'a>(
    program: &'a compiler::Program,
    node: &ast::Node,
) -> Option<&'a ast::SourceFile> {
    program
        .get_parsed_source_files_refs()
        .into_iter()
        .find(|file| {
            ast::get_source_file_of_node(file.store(), Some(*node))
                .is_some_and(|source_file| source_file == file.as_node())
        })
}

pub(crate) fn semantic_tokens_legend(
    client_capabilities: lsproto::ResolvedSemanticTokensClientCapabilities,
) -> lsproto::SemanticTokensLegend {
    // SemanticTokensLegend returns the legend describing the token types and modifiers.
    // It filters the legend to only include types and modifiers that the client supports,
    // as indicated by clientCapabilities.
    let mut types = Vec::with_capacity(TOKEN_TYPES.len());
    for token_type in TOKEN_TYPES {
        if client_capabilities
            .token_types
            .contains(&token_type.as_str().to_string())
        {
            types.push(token_type.clone());
        }
    }

    let mut modifiers = Vec::with_capacity(TOKEN_MODIFIERS.len());
    for modifier in TOKEN_MODIFIERS {
        if client_capabilities
            .token_modifiers
            .contains(&modifier.as_str().to_string())
        {
            modifiers.push(modifier.clone());
        }
    }

    lsproto::SemanticTokensLegend {
        token_types: types,
        token_modifiers: modifiers,
    }
}

impl LanguageService<'_> {
    pub fn provide_semantic_tokens(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
    ) -> Result<lsproto::SemanticTokensResponse, core::Error> {
        let (program, file) = self.get_program_and_file(document_uri);
        program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            file,
            |checker| {
                let tokens = self.collect_semantic_tokens(ctx, checker, file, program);

                if tokens.is_empty() {
                    return Ok(lsproto::SemanticTokensOrNull::default());
                }

                // Convert to LSP format (relative encoding)
                let encoded = encode_semantic_tokens(ctx, &tokens, file, &self.converters);
                Ok(lsproto::SemanticTokensOrNull {
                    semantic_tokens: Some(lsproto::SemanticTokens {
                        data: encoded,
                        ..Default::default()
                    }),
                    ..Default::default()
                })
            },
        )
    }

    pub fn provide_semantic_tokens_range(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
        range: lsproto::Range,
    ) -> Result<lsproto::SemanticTokensRangeResponse, core::Error> {
        let (program, file) = self.get_program_and_file(document_uri);
        program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            file,
            |checker| {
                let start = self
                    .converters
                    .line_and_character_to_position(file, range.start)
                    as i32;
                let end =
                    self.converters
                        .line_and_character_to_position(file, range.end) as i32;
                let tokens =
                    self.collect_semantic_tokens_in_range(ctx, checker, file, program, start, end);

                if tokens.is_empty() {
                    return Ok(lsproto::SemanticTokensOrNull::default());
                }

                // Convert to LSP format (relative encoding)
                let encoded = encode_semantic_tokens(ctx, &tokens, file, &self.converters);
                Ok(lsproto::SemanticTokensOrNull {
                    semantic_tokens: Some(lsproto::SemanticTokens {
                        data: encoded,
                        ..Default::default()
                    }),
                    ..Default::default()
                })
            },
        )
    }

    pub(crate) fn collect_semantic_tokens<'a>(
        &self,
        ctx: &core::Context,
        checker: &mut checker::Checker<'a, '_>,
        file: &'a ast::SourceFile,
        program: &'a compiler::Program,
    ) -> Vec<SemanticToken> {
        self.collect_semantic_tokens_in_range(ctx, checker, file, program, file.pos(), file.end())
    }

    pub(crate) fn collect_semantic_tokens_in_range<'a>(
        &self,
        ctx: &core::Context,
        checker: &mut checker::Checker<'a, '_>,
        file: &'a ast::SourceFile,
        program: &'a compiler::Program,
        span_start: i32,
        span_end: i32,
    ) -> Vec<SemanticToken> {
        let mut tokens = Vec::new();
        let mut in_jsx_element = false;

        fn visit<'a>(
            ctx: &core::Context,
            checker: &mut checker::Checker<'a, '_>,
            file: &'a ast::SourceFile,
            program: &'a compiler::Program,
            span_start: i32,
            span_end: i32,
            in_jsx_element: &mut bool,
            tokens: &mut Vec<SemanticToken>,
            node: ast::Node,
        ) -> bool {
            if ctx.err().is_some() {
                return false;
            }
            let store = file.store();
            if store.flags(node).intersects(ast::NodeFlags::Reparsed) {
                return false;
            }

            let node_loc = store.loc(node);
            let node_end = node_loc.end();
            if node_loc.pos() >= span_end || node_end <= span_start {
                return false;
            }

            let prev_in_jsx_element = *in_jsx_element;
            if ast::is_jsx_element(store, node) || ast::is_jsx_self_closing_element(store, node) {
                *in_jsx_element = true;
            } else if ast::is_jsx_expression(store, node) {
                *in_jsx_element = false;
            }

            if ast::is_identifier(store, node)
                && !store.text(node).is_empty()
                && !*in_jsx_element
                && !is_in_import_clause(store, &node)
                && !is_infinity_or_nan_string(&store.text(node))
            {
                let mut symbol = checker.get_symbol_at_location_public(node);
                if let Some(sym) = symbol.clone() {
                    if checker
                        .symbol_flags_public(sym)
                        .is_some_and(|flags| flags & ast::SYMBOL_FLAGS_ALIAS != 0)
                    {
                        symbol = checker.skip_alias_public(sym);
                    }
                }

                if let Some(symbol) = symbol {
                    let initial_token_type = {
                        classify_symbol(store, checker, symbol, get_meaning_from_location(node))
                    };
                    if let Some(mut token_type) = initial_token_type {
                        let mut token_modifier = TokenModifier::default();

                        if let Some(parent) = store.parent(node) {
                            let parent_is_declaration = ast::is_binding_element(store, parent)
                                || token_from_declaration_mapping(store.kind(parent))
                                    .is_some_and(|mapped| mapped == token_type);
                            if parent_is_declaration
                                && store.name(parent).is_some_and(|name| name == node)
                            {
                                token_modifier |= TokenModifier::DECLARATION;
                            }
                        }

                        if token_type == TokenType::Parameter
                            && ast::is_right_side_of_qualified_name_or_property_access(store, &node)
                        {
                            token_type = TokenType::Property;
                        }

                        token_type = reclassify_by_type(store, checker, &node, token_type);

                        let Some(symbol_flags) = checker.symbol_flags_public(symbol) else {
                            return false;
                        };
                        if let Some(decl) = checker.symbol_value_declaration_public(symbol) {
                            let decl_store = checker.source_file_store(decl).unwrap_or(store);
                            let modifiers = ast::get_combined_modifier_flags(decl_store, decl);
                            let node_flags = ast::get_combined_node_flags(decl_store, decl);

                            if modifiers.intersects(ast::ModifierFlags::Static) {
                                token_modifier |= TokenModifier::STATIC;
                            }
                            if modifiers.intersects(ast::ModifierFlags::Async) {
                                token_modifier |= TokenModifier::ASYNC;
                            }
                            if token_type != TokenType::Class
                                && token_type != TokenType::Interface
                                && (modifiers.intersects(ast::ModifierFlags::Readonly)
                                    || node_flags.intersects(ast::NodeFlags::Const)
                                    || symbol_flags & ast::SYMBOL_FLAGS_ENUM_MEMBER != 0)
                            {
                                token_modifier |= TokenModifier::READONLY;
                            }
                            if matches!(token_type, TokenType::Variable | TokenType::Function)
                                && is_local_declaration(decl_store, &decl, file)
                            {
                                token_modifier |= TokenModifier::LOCAL;
                            }

                            let decl_source_file = source_file_for_node(program, &decl);
                            if decl_source_file.as_ref().is_some_and(|file| {
                                program.is_source_file_default_library(file.path())
                            }) {
                                token_modifier |= TokenModifier::DEFAULT_LIBRARY;
                            }
                        } else {
                            for decl in checker.collect_symbol_declarations_public(symbol) {
                                let decl_source_file = source_file_for_node(program, &decl);
                                if decl_source_file.as_ref().is_some_and(|file| {
                                    program.is_source_file_default_library(file.path())
                                }) {
                                    token_modifier |= TokenModifier::DEFAULT_LIBRARY;
                                    break;
                                }
                            }
                        }

                        tokens.push(SemanticToken {
                            node,
                            token_type,
                            token_modifier,
                        });
                    }
                }
            }

            let _ = store.for_each_present_child(node, &mut |child: ast::Node| {
                visit(
                    ctx,
                    checker,
                    file,
                    program,
                    span_start,
                    span_end,
                    in_jsx_element,
                    tokens,
                    child,
                );
                std::ops::ControlFlow::Continue(())
            });
            *in_jsx_element = prev_in_jsx_element;
            false
        }

        visit(
            ctx,
            checker,
            file,
            program,
            span_start,
            span_end,
            &mut in_jsx_element,
            &mut tokens,
            file.as_node(),
        );

        if ctx.err().is_some() {
            return Vec::new();
        }

        tokens
    }
}

fn classify_symbol(
    store: &ast::AstStore,
    checker: &mut checker::Checker<'_, '_>,
    symbol: ast::SymbolIdentity,
    meaning: ast::SemanticMeaning,
) -> Option<TokenType> {
    let flags = checker.symbol_flags_public(symbol)?;
    if flags & ast::SYMBOL_FLAGS_CLASS != 0 {
        return Some(TokenType::Class);
    }
    if flags & ast::SYMBOL_FLAGS_ENUM != 0 {
        return Some(TokenType::Enum);
    }
    if flags & ast::SYMBOL_FLAGS_TYPE_ALIAS != 0 {
        return Some(TokenType::Type);
    }
    if flags & ast::SYMBOL_FLAGS_INTERFACE != 0 && meaning.0 & ast::SemanticMeaning::TYPE.0 != 0 {
        return Some(TokenType::Interface);
    }
    if flags & ast::SYMBOL_FLAGS_TYPE_PARAMETER != 0 {
        return Some(TokenType::TypeParameter);
    }

    let mut decl = checker.symbol_value_declaration_public(symbol);
    if decl.is_none() {
        decl = checker
            .collect_symbol_declarations_public(symbol)
            .first()
            .copied();
    }
    if let Some(decl) = decl {
        let mut decl_store = checker.source_file_store(decl).unwrap_or(store);
        if ast::is_binding_element(decl_store, decl) {
            let declaration = get_declaration_for_binding_element(decl_store, &decl);
            decl_store = checker.source_file_store(declaration).unwrap_or(decl_store);
            return token_from_declaration_mapping(decl_store.kind(declaration));
        }
        return token_from_declaration_mapping(decl_store.kind(decl));
    }

    None
}

fn token_from_declaration_mapping(kind: ast::Kind) -> Option<TokenType> {
    match kind {
        ast::Kind::VariableDeclaration => Some(TokenType::Variable),
        ast::Kind::Parameter => Some(TokenType::Parameter),
        ast::Kind::PropertyDeclaration => Some(TokenType::Property),
        ast::Kind::ModuleDeclaration => Some(TokenType::Namespace),
        ast::Kind::EnumDeclaration => Some(TokenType::Enum),
        ast::Kind::EnumMember => Some(TokenType::EnumMember),
        ast::Kind::ClassDeclaration | ast::Kind::ClassExpression => Some(TokenType::Class),
        ast::Kind::MethodDeclaration | ast::Kind::MethodSignature => Some(TokenType::Method),
        ast::Kind::FunctionDeclaration | ast::Kind::FunctionExpression => Some(TokenType::Function),
        ast::Kind::GetAccessor | ast::Kind::SetAccessor => Some(TokenType::Property),
        ast::Kind::PropertySignature => Some(TokenType::Property),
        ast::Kind::InterfaceDeclaration => Some(TokenType::Interface),
        ast::Kind::TypeAliasDeclaration => Some(TokenType::Type),
        ast::Kind::TypeParameter => Some(TokenType::TypeParameter),
        ast::Kind::PropertyAssignment | ast::Kind::ShorthandPropertyAssignment => {
            Some(TokenType::Property)
        }
        _ => None,
    }
}

fn reclassify_by_type<'a>(
    store: &ast::AstStore,
    checker: &mut checker::Checker<'a, '_>,
    node: &ast::Node,
    token_type: TokenType,
) -> TokenType {
    if !matches!(
        token_type,
        TokenType::Variable | TokenType::Property | TokenType::Parameter
    ) {
        return token_type;
    }

    let typ = checker.get_type_at_location(*node);
    let types = type_or_union_types(checker, typ);

    if token_type != TokenType::Parameter
        && types.iter().copied().any(|t| {
            !checker
                .get_signatures_of_type_public(t, checker::SIGNATURE_KIND_CONSTRUCT)
                .is_empty()
        })
    {
        return TokenType::Class;
    }

    let has_call_signatures = types.iter().copied().any(|t| {
        !checker
            .get_signatures_of_type_public(t, checker::SIGNATURE_KIND_CALL)
            .is_empty()
    });
    if has_call_signatures {
        let has_no_properties = !types
            .iter()
            .copied()
            .any(|t| !checker.structured_properties_public(t).is_empty());
        if has_no_properties || is_expression_in_call_expression(store, node) {
            if token_type == TokenType::Property {
                return TokenType::Method;
            }
            return TokenType::Function;
        }
    }

    token_type
}

fn type_or_union_types(
    checker: &checker::Checker<'_, '_>,
    typ: checker::TypeHandle,
) -> Vec<checker::TypeHandle> {
    if checker.type_flags_public(typ) & checker::TYPE_FLAGS_UNION != 0 {
        checker.type_types_public(typ)
    } else {
        vec![typ]
    }
}

fn is_local_declaration(
    store: &ast::AstStore,
    decl: &ast::Node,
    source_file: &ast::SourceFile,
) -> bool {
    let decl = if ast::is_binding_element(store, *decl) {
        get_declaration_for_binding_element(store, decl)
    } else {
        *decl
    };

    if ast::is_variable_declaration(store, decl) {
        if let Some(parent) = store.parent(decl) {
            if ast::is_catch_clause(store, parent) {
                return decl.store_id() == source_file.as_node().store_id();
            }
            if ast::is_variable_declaration_list(store, parent) {
                if let Some(grandparent) = store.parent(parent) {
                    return store.parent(grandparent).is_some_and(|great_grandparent| {
                        (!ast::is_source_file(store, great_grandparent)
                            || ast::is_catch_clause(store, grandparent))
                            && decl.store_id() == source_file.as_node().store_id()
                    });
                }
            }
        }
    } else if ast::is_function_declaration(store, decl) {
        return store.parent(decl).as_ref().is_some_and(|parent| {
            !ast::is_source_file(store, *parent)
                && decl.store_id() == source_file.as_node().store_id()
        });
    }

    false
}

fn get_declaration_for_binding_element(store: &ast::AstStore, element: &ast::Node) -> ast::Node {
    let mut element = *element;
    loop {
        let Some(parent) = store.parent(element) else {
            return element;
        };
        if ast::is_binding_pattern(store, parent) {
            if let Some(grandparent) = store.parent(parent) {
                if ast::is_binding_element(store, grandparent) {
                    element = grandparent;
                    continue;
                }
            }
            return store.parent(parent).unwrap_or(element);
        }
        return element;
    }
}

fn is_in_import_clause(store: &ast::AstStore, node: &ast::Node) -> bool {
    store.parent(*node).as_ref().is_some_and(|parent| {
        ast::is_import_clause(store, *parent)
            || ast::is_import_specifier(store, *parent)
            || ast::is_namespace_import(store, *parent)
    })
}

fn is_expression_in_call_expression(store: &ast::AstStore, node: &ast::Node) -> bool {
    let mut node = *node;
    while ast::is_right_side_of_qualified_name_or_property_access(store, &node) {
        node = store.parent(node).unwrap();
    }
    store.parent(node).as_ref().is_some_and(|parent| {
        ast::is_call_expression(store, *parent)
            && store
                .expression(*parent)
                .as_ref()
                .is_some_and(|expression| *expression == node)
    })
}

fn is_infinity_or_nan_string(text: &str) -> bool {
    text == "Infinity" || text == "NaN"
}

fn encode_semantic_tokens(
    ctx: &core::Context,
    tokens: &[SemanticToken],
    file: &ast::SourceFile,
    converters: &lsconv::Converters,
) -> Vec<u32> {
    // encodeSemanticTokens encodes tokens into the LSP format using relative positioning.
    // It filters tokens based on client capabilities, only including types and modifiers that the client supports.
    let caps = lsproto::get_client_capabilities(ctx);
    let client_capabilities = caps.text_document.semantic_tokens;

    // Build mapping from server token types/modifiers to client indices
    let mut type_mapping = std::collections::HashMap::new();
    let mut client_idx = 0_u32;
    // Map server token types to client-supported indices
    for (i, server_type) in TOKEN_TYPES.iter().enumerate() {
        if client_capabilities
            .token_types
            .contains(&server_type.as_str().to_string())
        {
            type_mapping.insert(i as u32, client_idx);
            client_idx += 1;
        }
    }

    let mut modifier_mapping = std::collections::HashMap::new();
    let mut client_bit = 0_u32;
    // Map server token modifiers to client-supported bit positions
    for modifier in TOKEN_MODIFIERS {
        if client_capabilities
            .token_modifiers
            .contains(&modifier.as_str().to_string())
        {
            modifier_mapping.insert(modifier.as_str().to_string(), client_bit);
            client_bit += 1;
        }
    }

    // Each token encodes 5 uint32 values: deltaLine, deltaChar, length, tokenType, tokenModifiers
    let mut encoded = Vec::with_capacity(tokens.len() * 5);
    let mut prev_line = 0_u32;
    let mut prev_char = 0_u32;

    for token in tokens {
        // Skip tokens with types not supported by the client
        let Some(client_type_idx) = type_mapping.get(&(token.token_type as u32)).copied() else {
            continue;
        };

        // Map modifiers to client-supported bit mask
        let mut client_modifier_mask = 0_u32;
        let server_modifiers = [
            (
                TokenModifier::DECLARATION,
                TOKEN_MODIFIERS[0].as_str().to_string(),
            ),
            (
                TokenModifier::DEFINITION,
                TOKEN_MODIFIERS[1].as_str().to_string(),
            ),
            (
                TokenModifier::READONLY,
                TOKEN_MODIFIERS[2].as_str().to_string(),
            ),
            (
                TokenModifier::STATIC,
                TOKEN_MODIFIERS[3].as_str().to_string(),
            ),
            (
                TokenModifier::DEPRECATED,
                TOKEN_MODIFIERS[4].as_str().to_string(),
            ),
            (
                TokenModifier::ABSTRACT,
                TOKEN_MODIFIERS[5].as_str().to_string(),
            ),
            (
                TokenModifier::ASYNC,
                TOKEN_MODIFIERS[6].as_str().to_string(),
            ),
            (
                TokenModifier::MODIFICATION,
                TOKEN_MODIFIERS[7].as_str().to_string(),
            ),
            (
                TokenModifier::DOCUMENTATION,
                TOKEN_MODIFIERS[8].as_str().to_string(),
            ),
            (
                TokenModifier::DEFAULT_LIBRARY,
                TOKEN_MODIFIERS[9].as_str().to_string(),
            ),
            (
                TokenModifier::LOCAL,
                TOKEN_MODIFIERS[10].as_str().to_string(),
            ),
        ];
        for (flag, modifier_name) in server_modifiers {
            if token.token_modifier.contains(flag) {
                if let Some(bit) = modifier_mapping.get(&modifier_name) {
                    client_modifier_mask |= 1 << bit;
                }
            }
        }

        // Use GetTokenPosOfNode to skip trivia (comments, whitespace) before the identifier
        let token_start = scanner::get_token_pos_of_node(&token.node, file, false);
        let token_end = file.store().loc(token.node).end();

        // Convert both start and end positions to LSP coordinates, then compute length
        let start_pos = converters.position_to_line_and_character(file, token_start as i32);
        let end_pos = converters.position_to_line_and_character(file, token_end);

        // Length is the character difference when on the same line
        let token_length = if start_pos.line == end_pos.line {
            end_pos.character - start_pos.character
        } else {
            panic!(
                "semantic tokens: token spans multiple lines: start=({},{}) end=({},{}) for token at offset {}",
                start_pos.line, start_pos.character, end_pos.line, end_pos.character, token_start
            )
        };

        let line = start_pos.line;
        let char_ = start_pos.character;
        // Verify that positions are strictly increasing (visitor walks in order)
        if !encoded.is_empty() && (line < prev_line || (line == prev_line && char_ <= prev_char)) {
            panic!(
                "semantic tokens: positions must be strictly increasing: prev=({},{}) current=({},{}) for token at offset {}",
                prev_line, prev_char, line, char_, token_start
            )
        }

        // Encode as: [deltaLine, deltaChar, length, tokenType, tokenModifiers]
        let delta_line = line - prev_line;
        let delta_char = if delta_line == 0 {
            char_ - prev_char
        } else {
            char_
        };

        encoded.extend_from_slice(&[
            delta_line,
            delta_char,
            token_length,
            client_type_idx,
            client_modifier_mask,
        ]);

        prev_line = line;
        prev_char = char_;
    }

    encoded
}
