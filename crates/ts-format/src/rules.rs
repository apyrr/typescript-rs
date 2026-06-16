use ts_ast as ast;

use crate::{RuleAction, RuleFlags, RuleSpec, TokenRange};

pub fn get_all_rules() -> Vec<RuleSpec> {
    let mut all_tokens =
        Vec::with_capacity((ast::Kind::LastToken as usize) - (ast::Kind::FirstToken as usize) + 1);
    let mut token = ast::Kind::FirstToken;
    while token <= ast::Kind::LastToken {
        if token != ast::Kind::EndOfFile {
            all_tokens.push(token);
        }
        token = token.next();
    }

    let any_token_except = |tokens: &[ast::Kind]| -> TokenRange {
        let mut new_tokens = Vec::with_capacity(all_tokens.len());
        for token in all_tokens.iter().copied() {
            if tokens.contains(&token) {
                continue;
            }
            new_tokens.push(token);
        }
        TokenRange {
            is_specific: false,
            tokens: new_tokens,
        }
    };

    let any_token = TokenRange {
        is_specific: false,
        tokens: all_tokens.clone(),
    };

    let any_token_including_multiline_comments =
        token_range_from_ex(&all_tokens, &[ast::Kind::MultiLineCommentTrivia]);
    let any_token_including_eof = token_range_from_ex(&all_tokens, &[ast::Kind::EndOfFile]);
    let keywords = token_range_from_range(ast::Kind::FirstKeyword, ast::Kind::LastKeyword);
    let binary_operators = token_range_from_range(
        ast::Kind::FirstBinaryOperator,
        ast::Kind::LastBinaryOperator,
    );
    let binary_keyword_operators = vec![
        ast::Kind::InKeyword,
        ast::Kind::InstanceOfKeyword,
        ast::Kind::OfKeyword,
        ast::Kind::AsKeyword,
        ast::Kind::IsKeyword,
        ast::Kind::SatisfiesKeyword,
    ];
    let unary_prefix_operators = vec![
        ast::Kind::PlusPlusToken,
        ast::Kind::MinusToken,
        ast::Kind::TildeToken,
        ast::Kind::ExclamationToken,
    ];
    let unary_prefix_expressions = vec![
        ast::Kind::NumericLiteral,
        ast::Kind::BigIntLiteral,
        ast::Kind::Identifier,
        ast::Kind::OpenParenToken,
        ast::Kind::OpenBracketToken,
        ast::Kind::OpenBraceToken,
        ast::Kind::ThisKeyword,
        ast::Kind::NewKeyword,
    ];
    let unary_preincrement_expressions = vec![
        ast::Kind::Identifier,
        ast::Kind::OpenParenToken,
        ast::Kind::ThisKeyword,
        ast::Kind::NewKeyword,
    ];
    let unary_postincrement_expressions = vec![
        ast::Kind::Identifier,
        ast::Kind::CloseParenToken,
        ast::Kind::CloseBracketToken,
        ast::Kind::NewKeyword,
    ];
    let unary_predecrement_expressions = vec![
        ast::Kind::Identifier,
        ast::Kind::OpenParenToken,
        ast::Kind::ThisKeyword,
        ast::Kind::NewKeyword,
    ];
    let unary_postdecrement_expressions = vec![
        ast::Kind::Identifier,
        ast::Kind::CloseParenToken,
        ast::Kind::CloseBracketToken,
        ast::Kind::NewKeyword,
    ];
    let comments = vec![
        ast::Kind::SingleLineCommentTrivia,
        ast::Kind::MultiLineCommentTrivia,
    ];
    let type_keywords = vec![
        ast::Kind::AnyKeyword,
        ast::Kind::AssertsKeyword,
        ast::Kind::BigIntKeyword,
        ast::Kind::BooleanKeyword,
        ast::Kind::FalseKeyword,
        ast::Kind::InferKeyword,
        ast::Kind::KeyOfKeyword,
        ast::Kind::NeverKeyword,
        ast::Kind::NullKeyword,
        ast::Kind::NumberKeyword,
        ast::Kind::ObjectKeyword,
        ast::Kind::ReadonlyKeyword,
        ast::Kind::StringKeyword,
        ast::Kind::SymbolKeyword,
        ast::Kind::TypeOfKeyword,
        ast::Kind::TrueKeyword,
        ast::Kind::VoidKeyword,
        ast::Kind::UndefinedKeyword,
        ast::Kind::UniqueKeyword,
        ast::Kind::UnknownKeyword,
    ];
    let mut type_names = vec![ast::Kind::Identifier];
    type_names.extend(type_keywords);

    // Place a space before open brace in a function declaration
    // TypeScript: Function can have return types, which can be made of tons of different token kinds
    let function_open_brace_left_token_range = any_token_including_multiline_comments.clone();

    // Place a space before open brace in a TypeScript declaration that has braces as children (class, module, enum, etc)
    let type_script_open_brace_left_token_range = token_range_from(&[
        ast::Kind::Identifier,
        ast::Kind::GreaterThanToken,
        ast::Kind::MultiLineCommentTrivia,
        ast::Kind::ClassKeyword,
        ast::Kind::ExportKeyword,
        ast::Kind::ImportKeyword,
    ]);

    // Place a space before open brace in a control flow construct
    let control_open_brace_left_token_range = token_range_from(&[
        ast::Kind::CloseParenToken,
        ast::Kind::MultiLineCommentTrivia,
        ast::Kind::DoKeyword,
        ast::Kind::TryKeyword,
        ast::Kind::FinallyKeyword,
        ast::Kind::ElseKeyword,
        ast::Kind::CatchKeyword,
    ]);

    // These rules are higher in priority than user-configurable
    let high_priority_common_rules = vec![
        // Leave comments alone
        entry(
            "IgnoreBeforeComment",
            any_token.clone(),
            range(comments.clone()),
            RuleAction::STOP_PROCESSING_SPACE_ACTIONS,
        ),
        entry(
            "IgnoreAfterLineComment",
            kind(ast::Kind::SingleLineCommentTrivia),
            any_token.clone(),
            RuleAction::STOP_PROCESSING_SPACE_ACTIONS,
        ),
        entry(
            "NotSpaceBeforeColon",
            any_token.clone(),
            kind(ast::Kind::ColonToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "SpaceAfterColon",
            kind(ast::Kind::ColonToken),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceBeforeQuestionMark",
            any_token.clone(),
            kind(ast::Kind::QuestionToken),
            RuleAction::DELETE_SPACE,
        ),
        // insert space after '?' only when it is used in conditional operator
        entry(
            "SpaceAfterQuestionMarkInConditionalOperator",
            kind(ast::Kind::QuestionToken),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        // in other cases there should be no space between '?' and next token
        entry(
            "NoSpaceAfterQuestionMark",
            kind(ast::Kind::QuestionToken),
            any_token.clone(),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceBeforeDot",
            any_token.clone(),
            range(vec![ast::Kind::DotToken, ast::Kind::QuestionDotToken]),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceAfterDot",
            range(vec![ast::Kind::DotToken, ast::Kind::QuestionDotToken]),
            any_token.clone(),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceBetweenImportParenInImportType",
            kind(ast::Kind::ImportKeyword),
            kind(ast::Kind::OpenParenToken),
            RuleAction::DELETE_SPACE,
        ),
        // Special handling of unary operators.
        // Prefix operators generally shouldn't have a space between
        // them and their target unary expression.
        entry(
            "NoSpaceAfterUnaryPrefixOperator",
            range(unary_prefix_operators),
            range(unary_prefix_expressions),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceAfterUnaryPreincrementOperator",
            kind(ast::Kind::PlusPlusToken),
            range(unary_preincrement_expressions),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceAfterUnaryPredecrementOperator",
            kind(ast::Kind::MinusMinusToken),
            range(unary_predecrement_expressions),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceBeforeUnaryPostincrementOperator",
            range(unary_postincrement_expressions),
            kind(ast::Kind::PlusPlusToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceBeforeUnaryPostdecrementOperator",
            range(unary_postdecrement_expressions),
            kind(ast::Kind::MinusMinusToken),
            RuleAction::DELETE_SPACE,
        ),
        // More unary operator special-casing.
        // DevDiv 181814: Be careful when removing leading whitespace
        // around unary operators.  Examples:
        //      1 - -2  --X--> 1--2
        //      a + ++b --X--> a+++b
        entry(
            "SpaceAfterPostincrementWhenFollowedByAdd",
            kind(ast::Kind::PlusPlusToken),
            kind(ast::Kind::PlusToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceAfterAddWhenFollowedByUnaryPlus",
            kind(ast::Kind::PlusToken),
            kind(ast::Kind::PlusToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceAfterAddWhenFollowedByPreincrement",
            kind(ast::Kind::PlusToken),
            kind(ast::Kind::PlusPlusToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceAfterPostdecrementWhenFollowedBySubtract",
            kind(ast::Kind::MinusMinusToken),
            kind(ast::Kind::MinusToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceAfterSubtractWhenFollowedByUnaryMinus",
            kind(ast::Kind::MinusToken),
            kind(ast::Kind::MinusToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceAfterSubtractWhenFollowedByPredecrement",
            kind(ast::Kind::MinusToken),
            kind(ast::Kind::MinusMinusToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceAfterCloseBrace",
            kind(ast::Kind::CloseBraceToken),
            range(vec![ast::Kind::CommaToken, ast::Kind::SemicolonToken]),
            RuleAction::DELETE_SPACE,
        ),
        // For functions and control block place } on a new line []ast.Kind{multi-line rule}
        entry(
            "NewLineBeforeCloseBraceInBlockContext",
            any_token_including_multiline_comments.clone(),
            kind(ast::Kind::CloseBraceToken),
            RuleAction::INSERT_NEW_LINE,
        ),
        // Space/new line after }.
        entry(
            "SpaceAfterCloseBrace",
            kind(ast::Kind::CloseBraceToken),
            any_token_except(&[ast::Kind::CloseParenToken]),
            RuleAction::INSERT_SPACE,
        ),
        // Special case for (}, else) and (}, while) since else & while tokens are not part of the tree which makes SpaceAfterCloseBrace rule not applied
        // Also should not apply to })
        entry(
            "SpaceBetweenCloseBraceAndElse",
            kind(ast::Kind::CloseBraceToken),
            kind(ast::Kind::ElseKeyword),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceBetweenCloseBraceAndWhile",
            kind(ast::Kind::CloseBraceToken),
            kind(ast::Kind::WhileKeyword),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceBetweenEmptyBraceBrackets",
            kind(ast::Kind::OpenBraceToken),
            kind(ast::Kind::CloseBraceToken),
            RuleAction::DELETE_SPACE,
        ),
        // Add a space after control dec context if the next character is an open bracket ex: 'if (false)[]ast.Kind{a, b} = []ast.Kind{1, 2};' -> 'if (false) []ast.Kind{a, b} = []ast.Kind{1, 2};'
        entry(
            "SpaceAfterConditionalClosingParen",
            kind(ast::Kind::CloseParenToken),
            kind(ast::Kind::OpenBracketToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceBetweenFunctionKeywordAndStar",
            kind(ast::Kind::FunctionKeyword),
            kind(ast::Kind::AsteriskToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "SpaceAfterStarInGeneratorDeclaration",
            kind(ast::Kind::AsteriskToken),
            kind(ast::Kind::Identifier),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceAfterFunctionInFuncDecl",
            kind(ast::Kind::FunctionKeyword),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        // Insert new line after { and before } in multi-line contexts.
        entry(
            "NewLineAfterOpenBraceInBlockContext",
            kind(ast::Kind::OpenBraceToken),
            any_token.clone(),
            RuleAction::INSERT_NEW_LINE,
        ),
        // For get/set members, we check for (identifier,identifier) since get/set don't have tokens and they are represented as just an identifier token.
        // Though, we do extra check on the context to make sure we are dealing with get/set node. Example:
        //      get x() {}
        //      set x(val) {}
        entry(
            "SpaceAfterGetSetInMember",
            range(vec![ast::Kind::GetKeyword, ast::Kind::SetKeyword]),
            kind(ast::Kind::Identifier),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceBetweenYieldKeywordAndStar",
            kind(ast::Kind::YieldKeyword),
            kind(ast::Kind::AsteriskToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "SpaceBetweenYieldOrYieldStarAndOperand",
            range(vec![ast::Kind::YieldKeyword, ast::Kind::AsteriskToken]),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceBetweenReturnAndSemicolon",
            kind(ast::Kind::ReturnKeyword),
            kind(ast::Kind::SemicolonToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "SpaceAfterCertainKeywords",
            range(vec![
                ast::Kind::VarKeyword,
                ast::Kind::ThrowKeyword,
                ast::Kind::NewKeyword,
                ast::Kind::DeleteKeyword,
                ast::Kind::ReturnKeyword,
                ast::Kind::TypeOfKeyword,
                ast::Kind::AwaitKeyword,
            ]),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceAfterLetConstInVariableDeclaration",
            range(vec![ast::Kind::LetKeyword, ast::Kind::ConstKeyword]),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceBeforeOpenParenInFuncCall",
            any_token.clone(),
            kind(ast::Kind::OpenParenToken),
            RuleAction::DELETE_SPACE,
        ),
        // Special case for binary operators (that are keywords). For these we have to add a space and shouldn't follow any user options.
        entry(
            "SpaceBeforeBinaryKeywordOperator",
            any_token.clone(),
            range(binary_keyword_operators.clone()),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceAfterBinaryKeywordOperator",
            range(binary_keyword_operators),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceAfterVoidOperator",
            kind(ast::Kind::VoidKeyword),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        // Async-await
        entry(
            "SpaceBetweenAsyncAndOpenParen",
            kind(ast::Kind::AsyncKeyword),
            kind(ast::Kind::OpenParenToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceBetweenAsyncAndFunctionKeyword",
            kind(ast::Kind::AsyncKeyword),
            range(vec![ast::Kind::FunctionKeyword, ast::Kind::Identifier]),
            RuleAction::INSERT_SPACE,
        ),
        // Template string
        entry(
            "NoSpaceBetweenTagAndTemplateString",
            range(vec![ast::Kind::Identifier, ast::Kind::CloseParenToken]),
            range(vec![
                ast::Kind::NoSubstitutionTemplateLiteral,
                ast::Kind::TemplateHead,
            ]),
            RuleAction::DELETE_SPACE,
        ),
        // JSX opening elements
        entry(
            "SpaceBeforeJsxAttribute",
            any_token.clone(),
            kind(ast::Kind::Identifier),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceBeforeSlashInJsxOpeningElement",
            any_token.clone(),
            kind(ast::Kind::SlashToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceBeforeGreaterThanTokenInJsxOpeningElement",
            kind(ast::Kind::SlashToken),
            kind(ast::Kind::GreaterThanToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceBeforeEqualInJsxAttribute",
            any_token.clone(),
            kind(ast::Kind::EqualsToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceAfterEqualInJsxAttribute",
            kind(ast::Kind::EqualsToken),
            any_token.clone(),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceBeforeJsxNamespaceColon",
            kind(ast::Kind::Identifier),
            kind(ast::Kind::ColonToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceAfterJsxNamespaceColon",
            kind(ast::Kind::ColonToken),
            kind(ast::Kind::Identifier),
            RuleAction::DELETE_SPACE,
        ),
        // TypeScript-specific rules
        // Use of module as a function call. e.g.: import m2 = module("m2");
        entry(
            "NoSpaceAfterModuleImport",
            range(vec![ast::Kind::ModuleKeyword, ast::Kind::RequireKeyword]),
            kind(ast::Kind::OpenParenToken),
            RuleAction::DELETE_SPACE,
        ),
        // Add a space around certain TypeScript keywords
        entry(
            "SpaceAfterCertainTypeScriptKeywords",
            range(vec![
                ast::Kind::AbstractKeyword,
                ast::Kind::AccessorKeyword,
                ast::Kind::ClassKeyword,
                ast::Kind::DeclareKeyword,
                ast::Kind::DefaultKeyword,
                ast::Kind::EnumKeyword,
                ast::Kind::ExportKeyword,
                ast::Kind::ExtendsKeyword,
                ast::Kind::GetKeyword,
                ast::Kind::ImplementsKeyword,
                ast::Kind::ImportKeyword,
                ast::Kind::InterfaceKeyword,
                ast::Kind::ModuleKeyword,
                ast::Kind::NamespaceKeyword,
                ast::Kind::OverrideKeyword,
                ast::Kind::PrivateKeyword,
                ast::Kind::PublicKeyword,
                ast::Kind::ProtectedKeyword,
                ast::Kind::ReadonlyKeyword,
                ast::Kind::SetKeyword,
                ast::Kind::StaticKeyword,
                ast::Kind::TypeKeyword,
                ast::Kind::FromKeyword,
                ast::Kind::KeyOfKeyword,
                ast::Kind::InferKeyword,
            ]),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceBeforeCertainTypeScriptKeywords",
            any_token.clone(),
            range(vec![
                ast::Kind::ExtendsKeyword,
                ast::Kind::ImplementsKeyword,
                ast::Kind::FromKeyword,
            ]),
            RuleAction::INSERT_SPACE,
        ),
        // Treat string literals in module names as identifiers, and add a space between the literal and the opening Brace braces, e.g.: module "m2" {
        entry(
            "SpaceAfterModuleName",
            kind(ast::Kind::StringLiteral),
            kind(ast::Kind::OpenBraceToken),
            RuleAction::INSERT_SPACE,
        ),
        // Lambda expressions
        entry(
            "SpaceBeforeArrow",
            any_token.clone(),
            kind(ast::Kind::EqualsGreaterThanToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceAfterArrow",
            kind(ast::Kind::EqualsGreaterThanToken),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        // Optional parameters and let args
        entry(
            "NoSpaceAfterEllipsis",
            kind(ast::Kind::DotDotDotToken),
            kind(ast::Kind::Identifier),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceAfterOptionalParameters",
            kind(ast::Kind::QuestionToken),
            range(vec![ast::Kind::CloseParenToken, ast::Kind::CommaToken]),
            RuleAction::DELETE_SPACE,
        ),
        // Remove spaces in empty interface literals. e.g.: x: {}
        entry(
            "NoSpaceBetweenEmptyInterfaceBraceBrackets",
            kind(ast::Kind::OpenBraceToken),
            kind(ast::Kind::CloseBraceToken),
            RuleAction::DELETE_SPACE,
        ),
        // generics and type assertions
        entry(
            "NoSpaceBeforeOpenAngularBracket",
            range(type_names.clone()),
            kind(ast::Kind::LessThanToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceBetweenCloseParenAndAngularBracket",
            kind(ast::Kind::CloseParenToken),
            kind(ast::Kind::LessThanToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceAfterOpenAngularBracket",
            kind(ast::Kind::LessThanToken),
            any_token.clone(),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceBeforeCloseAngularBracket",
            any_token.clone(),
            kind(ast::Kind::GreaterThanToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceAfterCloseAngularBracket",
            kind(ast::Kind::GreaterThanToken),
            range(vec![
                ast::Kind::OpenParenToken,
                ast::Kind::OpenBracketToken,
                ast::Kind::GreaterThanToken,
                ast::Kind::CommaToken,
            ]),
            RuleAction::DELETE_SPACE,
        ),
        // decorators
        entry(
            "SpaceBeforeAt",
            range(vec![ast::Kind::CloseParenToken, ast::Kind::Identifier]),
            kind(ast::Kind::AtToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceAfterAt",
            kind(ast::Kind::AtToken),
            any_token.clone(),
            RuleAction::DELETE_SPACE,
        ),
        // Insert space after @ in decorator
        entry(
            "SpaceAfterDecorator",
            any_token.clone(),
            range(vec![
                ast::Kind::AbstractKeyword,
                ast::Kind::Identifier,
                ast::Kind::ExportKeyword,
                ast::Kind::DefaultKeyword,
                ast::Kind::ClassKeyword,
                ast::Kind::StaticKeyword,
                ast::Kind::PublicKeyword,
                ast::Kind::PrivateKeyword,
                ast::Kind::ProtectedKeyword,
                ast::Kind::GetKeyword,
                ast::Kind::SetKeyword,
                ast::Kind::OpenBracketToken,
                ast::Kind::AsteriskToken,
            ]),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceBeforeNonNullAssertionOperator",
            any_token.clone(),
            kind(ast::Kind::ExclamationToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceAfterNewKeywordOnConstructorSignature",
            kind(ast::Kind::NewKeyword),
            kind(ast::Kind::OpenParenToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "SpaceLessThanAndNonJSXTypeAnnotation",
            kind(ast::Kind::LessThanToken),
            kind(ast::Kind::LessThanToken),
            RuleAction::INSERT_SPACE,
        ),
    ];

    // These rules are applied after high priority
    let user_configurable_rules = vec![
        // Treat constructor as an identifier in a function declaration, and remove spaces between constructor and following left parentheses
        entry(
            "SpaceAfterConstructor",
            kind(ast::Kind::ConstructorKeyword),
            kind(ast::Kind::OpenParenToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceAfterConstructor",
            kind(ast::Kind::ConstructorKeyword),
            kind(ast::Kind::OpenParenToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "SpaceAfterComma",
            kind(ast::Kind::CommaToken),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceAfterComma",
            kind(ast::Kind::CommaToken),
            any_token.clone(),
            RuleAction::DELETE_SPACE,
        ),
        // Insert space after function keyword for anonymous functions
        entry(
            "SpaceAfterAnonymousFunctionKeyword",
            range(vec![ast::Kind::FunctionKeyword, ast::Kind::AsteriskToken]),
            kind(ast::Kind::OpenParenToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceAfterAnonymousFunctionKeyword",
            range(vec![ast::Kind::FunctionKeyword, ast::Kind::AsteriskToken]),
            kind(ast::Kind::OpenParenToken),
            RuleAction::DELETE_SPACE,
        ),
        // Insert space after keywords in control flow statements
        entry(
            "SpaceAfterKeywordInControl",
            keywords.clone(),
            kind(ast::Kind::OpenParenToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceAfterKeywordInControl",
            keywords,
            kind(ast::Kind::OpenParenToken),
            RuleAction::DELETE_SPACE,
        ),
        // Insert space after opening and before closing nonempty parenthesis
        entry(
            "SpaceAfterOpenParen",
            kind(ast::Kind::OpenParenToken),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceBeforeCloseParen",
            any_token.clone(),
            kind(ast::Kind::CloseParenToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceBetweenOpenParens",
            kind(ast::Kind::OpenParenToken),
            kind(ast::Kind::OpenParenToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceBetweenParens",
            kind(ast::Kind::OpenParenToken),
            kind(ast::Kind::CloseParenToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceAfterOpenParen",
            kind(ast::Kind::OpenParenToken),
            any_token.clone(),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceBeforeCloseParen",
            any_token.clone(),
            kind(ast::Kind::CloseParenToken),
            RuleAction::DELETE_SPACE,
        ),
        // Insert space after opening and before closing nonempty brackets
        entry(
            "SpaceAfterOpenBracket",
            kind(ast::Kind::OpenBracketToken),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceBeforeCloseBracket",
            any_token.clone(),
            kind(ast::Kind::CloseBracketToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceBetweenBrackets",
            kind(ast::Kind::OpenBracketToken),
            kind(ast::Kind::CloseBracketToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceAfterOpenBracket",
            kind(ast::Kind::OpenBracketToken),
            any_token.clone(),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceBeforeCloseBracket",
            any_token.clone(),
            kind(ast::Kind::CloseBracketToken),
            RuleAction::DELETE_SPACE,
        ),
        // Insert a space after { and before } in single-line contexts, but remove space from empty object literals {}.
        entry(
            "SpaceAfterOpenBrace",
            kind(ast::Kind::OpenBraceToken),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceBeforeCloseBrace",
            any_token.clone(),
            kind(ast::Kind::CloseBraceToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceBetweenEmptyBraceBrackets",
            kind(ast::Kind::OpenBraceToken),
            kind(ast::Kind::CloseBraceToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceAfterOpenBrace",
            kind(ast::Kind::OpenBraceToken),
            any_token.clone(),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceBeforeCloseBrace",
            any_token.clone(),
            kind(ast::Kind::CloseBraceToken),
            RuleAction::DELETE_SPACE,
        ),
        // Insert a space after opening and before closing empty brace brackets
        entry(
            "SpaceBetweenEmptyBraceBrackets",
            kind(ast::Kind::OpenBraceToken),
            kind(ast::Kind::CloseBraceToken),
            RuleAction::INSERT_SPACE,
        ),
        entry_contexts(
            "NoSpaceBetweenEmptyBraceBrackets",
            kind(ast::Kind::OpenBraceToken),
            kind(ast::Kind::CloseBraceToken),
            RuleAction::DELETE_SPACE,
            &[
                "isOptionDisabled(insertSpaceAfterOpeningAndBeforeClosingEmptyBracesOption)",
                "isNonJsxSameLineTokenContext",
            ],
        ),
        // Insert space after opening and before closing template string braces
        entry_flags(
            "SpaceAfterTemplateHeadAndMiddle",
            range(vec![ast::Kind::TemplateHead, ast::Kind::TemplateMiddle]),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
            RuleFlags::CAN_DELETE_NEW_LINES,
        ),
        entry(
            "SpaceBeforeTemplateMiddleAndTail",
            any_token.clone(),
            range(vec![ast::Kind::TemplateMiddle, ast::Kind::TemplateTail]),
            RuleAction::INSERT_SPACE,
        ),
        entry_flags(
            "NoSpaceAfterTemplateHeadAndMiddle",
            range(vec![ast::Kind::TemplateHead, ast::Kind::TemplateMiddle]),
            any_token.clone(),
            RuleAction::DELETE_SPACE,
            RuleFlags::CAN_DELETE_NEW_LINES,
        ),
        entry(
            "NoSpaceBeforeTemplateMiddleAndTail",
            any_token.clone(),
            range(vec![ast::Kind::TemplateMiddle, ast::Kind::TemplateTail]),
            RuleAction::DELETE_SPACE,
        ),
        // No space after { and before } in JSX expression
        entry(
            "SpaceAfterOpenBraceInJsxExpression",
            kind(ast::Kind::OpenBraceToken),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceBeforeCloseBraceInJsxExpression",
            any_token.clone(),
            kind(ast::Kind::CloseBraceToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceAfterOpenBraceInJsxExpression",
            kind(ast::Kind::OpenBraceToken),
            any_token.clone(),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceBeforeCloseBraceInJsxExpression",
            any_token.clone(),
            kind(ast::Kind::CloseBraceToken),
            RuleAction::DELETE_SPACE,
        ),
        // Insert space after semicolon in for statement
        entry(
            "SpaceAfterSemicolonInFor",
            kind(ast::Kind::SemicolonToken),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceAfterSemicolonInFor",
            kind(ast::Kind::SemicolonToken),
            any_token.clone(),
            RuleAction::DELETE_SPACE,
        ),
        // Insert space before and after binary operators
        entry(
            "SpaceBeforeBinaryOperator",
            any_token.clone(),
            binary_operators.clone(),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "SpaceAfterBinaryOperator",
            binary_operators.clone(),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceBeforeBinaryOperator",
            any_token.clone(),
            binary_operators.clone(),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceAfterBinaryOperator",
            binary_operators,
            any_token.clone(),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "SpaceBeforeOpenParenInFuncDecl",
            any_token.clone(),
            kind(ast::Kind::OpenParenToken),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceBeforeOpenParenInFuncDecl",
            any_token.clone(),
            kind(ast::Kind::OpenParenToken),
            RuleAction::DELETE_SPACE,
        ),
        // Open Brace braces after control block
        entry_flags(
            "NewLineBeforeOpenBraceInControl",
            control_open_brace_left_token_range.clone(),
            kind(ast::Kind::OpenBraceToken),
            RuleAction::INSERT_NEW_LINE,
            RuleFlags::CAN_DELETE_NEW_LINES,
        ),
        // Open Brace braces after function
        // TypeScript: Function can have return types, which can be made of tons of different token kinds
        entry_flags(
            "NewLineBeforeOpenBraceInFunction",
            function_open_brace_left_token_range.clone(),
            kind(ast::Kind::OpenBraceToken),
            RuleAction::INSERT_NEW_LINE,
            RuleFlags::CAN_DELETE_NEW_LINES,
        ),
        // Open Brace braces after TypeScript module/class/interface
        entry_flags(
            "NewLineBeforeOpenBraceInTypeScriptDeclWithBlock",
            type_script_open_brace_left_token_range.clone(),
            kind(ast::Kind::OpenBraceToken),
            RuleAction::INSERT_NEW_LINE,
            RuleFlags::CAN_DELETE_NEW_LINES,
        ),
        entry(
            "SpaceAfterTypeAssertion",
            kind(ast::Kind::GreaterThanToken),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceAfterTypeAssertion",
            kind(ast::Kind::GreaterThanToken),
            any_token.clone(),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "SpaceBeforeTypeAnnotation",
            any_token.clone(),
            range(vec![ast::Kind::QuestionToken, ast::Kind::ColonToken]),
            RuleAction::INSERT_SPACE,
        ),
        entry(
            "NoSpaceBeforeTypeAnnotation",
            any_token.clone(),
            range(vec![ast::Kind::QuestionToken, ast::Kind::ColonToken]),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoOptionalSemicolon",
            kind(ast::Kind::SemicolonToken),
            any_token_including_eof.clone(),
            RuleAction::DELETE_TOKEN,
        ),
        entry(
            "OptionalSemicolon",
            any_token.clone(),
            any_token_including_eof,
            RuleAction::INSERT_TRAILING_SEMICOLON,
        ),
    ];

    // These rules are lower in priority than user-configurable. Rules earlier in this list have priority over rules later in the list.
    let low_priority_common_rules = vec![
        // Space after keyword but not before ; or : or ?
        entry(
            "NoSpaceBeforeSemicolon",
            any_token.clone(),
            kind(ast::Kind::SemicolonToken),
            RuleAction::DELETE_SPACE,
        ),
        entry_flags(
            "SpaceBeforeOpenBraceInControl",
            control_open_brace_left_token_range,
            kind(ast::Kind::OpenBraceToken),
            RuleAction::INSERT_SPACE,
            RuleFlags::CAN_DELETE_NEW_LINES,
        ),
        entry_flags(
            "SpaceBeforeOpenBraceInFunction",
            function_open_brace_left_token_range,
            kind(ast::Kind::OpenBraceToken),
            RuleAction::INSERT_SPACE,
            RuleFlags::CAN_DELETE_NEW_LINES,
        ),
        entry_flags(
            "SpaceBeforeOpenBraceInTypeScriptDeclWithBlock",
            type_script_open_brace_left_token_range,
            kind(ast::Kind::OpenBraceToken),
            RuleAction::INSERT_SPACE,
            RuleFlags::CAN_DELETE_NEW_LINES,
        ),
        entry(
            "NoSpaceBeforeComma",
            any_token.clone(),
            kind(ast::Kind::CommaToken),
            RuleAction::DELETE_SPACE,
        ),
        // No space before and after indexer `x[]ast.Kind{}`
        entry(
            "NoSpaceBeforeOpenBracket",
            any_token_except(&[ast::Kind::AsyncKeyword, ast::Kind::CaseKeyword]),
            kind(ast::Kind::OpenBracketToken),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "NoSpaceAfterCloseBracket",
            kind(ast::Kind::CloseBracketToken),
            any_token.clone(),
            RuleAction::DELETE_SPACE,
        ),
        entry(
            "SpaceAfterSemicolon",
            kind(ast::Kind::SemicolonToken),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        // Remove extra space between for and await
        entry(
            "SpaceBetweenForAndAwaitKeyword",
            kind(ast::Kind::ForKeyword),
            kind(ast::Kind::AwaitKeyword),
            RuleAction::INSERT_SPACE,
        ),
        // Remove extra spaces between ... and type name in tuple spread
        entry(
            "SpaceBetweenDotDotDotAndTypeName",
            kind(ast::Kind::DotDotDotToken),
            range(type_names),
            RuleAction::DELETE_SPACE,
        ),
        // Add a space between statements. All keywords except (do,else,case) has open/close parens after them.
        // So, we have a rule to add a space for []ast.Kind{),Any}, []ast.Kind{do,Any}, []ast.Kind{else,Any}, and []ast.Kind{case,Any}
        entry(
            "SpaceBetweenStatements",
            range(vec![
                ast::Kind::CloseParenToken,
                ast::Kind::DoKeyword,
                ast::Kind::ElseKeyword,
                ast::Kind::CaseKeyword,
            ]),
            any_token.clone(),
            RuleAction::INSERT_SPACE,
        ),
        // This low-pri rule takes care of "try {", "catch {" and "finally {" in case the rule SpaceBeforeOpenBraceInControl didn't execute on FormatOnEnter.
        entry(
            "SpaceAfterTryCatchFinally",
            range(vec![
                ast::Kind::TryKeyword,
                ast::Kind::CatchKeyword,
                ast::Kind::FinallyKeyword,
            ]),
            kind(ast::Kind::OpenBraceToken),
            RuleAction::INSERT_SPACE,
        ),
    ];

    let mut result = Vec::with_capacity(
        high_priority_common_rules.len()
            + user_configurable_rules.len()
            + low_priority_common_rules.len(),
    );
    result.extend(high_priority_common_rules);
    result.extend(user_configurable_rules);
    result.extend(low_priority_common_rules);
    result
}

pub fn token_range_from(tokens: &[ast::Kind]) -> TokenRange {
    TokenRange {
        is_specific: true,
        tokens: tokens.to_vec(),
    }
}

pub fn token_range_from_ex(prefix: &[ast::Kind], tokens: &[ast::Kind]) -> TokenRange {
    let mut combined = prefix.to_vec();
    combined.extend_from_slice(tokens);
    TokenRange {
        is_specific: true,
        tokens: combined,
    }
}

pub fn token_range_from_range(start: ast::Kind, end: ast::Kind) -> TokenRange {
    let mut tokens = Vec::with_capacity((end as usize) - (start as usize) + 1);
    let mut token = start;
    while token <= end {
        tokens.push(token);
        token = token.next();
    }
    token_range_from(&tokens)
}

fn kind(token: ast::Kind) -> TokenRange {
    token_range_from(&[token])
}

fn range(tokens: Vec<ast::Kind>) -> TokenRange {
    token_range_from(&tokens)
}

fn entry(debug_name: &str, left: TokenRange, right: TokenRange, action: RuleAction) -> RuleSpec {
    entry_flags(debug_name, left, right, action, RuleFlags::NONE)
}

fn entry_flags(
    debug_name: &str,
    left: TokenRange,
    right: TokenRange,
    action: RuleAction,
    flags: RuleFlags,
) -> RuleSpec {
    entry_contexts_flags(
        debug_name,
        left,
        right,
        action,
        flags,
        rule_contexts_for(debug_name),
    )
}

fn entry_contexts(
    debug_name: &str,
    left: TokenRange,
    right: TokenRange,
    action: RuleAction,
    context_names: &[&str],
) -> RuleSpec {
    entry_contexts_flags(
        debug_name,
        left,
        right,
        action,
        RuleFlags::NONE,
        context_names,
    )
}

fn entry_contexts_flags(
    debug_name: &str,
    left: TokenRange,
    right: TokenRange,
    action: RuleAction,
    flags: RuleFlags,
    context_names: &[&str],
) -> RuleSpec {
    let context_names = context_names
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    RuleSpec {
        left_token_range: left,
        right_token_range: right,
        rule: crate::RuleImpl {
            debug_name: debug_name.to_string(),
            context: Vec::new(),
            context_names,
            action,
            flags,
        },
    }
}

fn rule_contexts_for(debug_name: &str) -> &'static [&'static str] {
    RULE_CONTEXTS
        .iter()
        .find_map(|(name, contexts)| (*name == debug_name).then_some(*contexts))
        .unwrap_or(&[])
}

const RULE_CONTEXTS: &[(&str, &[&str])] = &[
    ("IgnoreBeforeComment", &[]),
    ("IgnoreAfterLineComment", &[]),
    (
        "NotSpaceBeforeColon",
        &[
            "isNonJsxSameLineTokenContext",
            "isNotBinaryOpContext",
            "isNotTypeAnnotationContext",
        ],
    ),
    (
        "SpaceAfterColon",
        &[
            "isNonJsxSameLineTokenContext",
            "isNotBinaryOpContext",
            "isNextTokenParentNotJsxNamespacedName",
        ],
    ),
    (
        "NoSpaceBeforeQuestionMark",
        &[
            "isNonJsxSameLineTokenContext",
            "isNotBinaryOpContext",
            "isNotTypeAnnotationContext",
        ],
    ),
    (
        "SpaceAfterQuestionMarkInConditionalOperator",
        &[
            "isNonJsxSameLineTokenContext",
            "isConditionalOperatorContext",
        ],
    ),
    (
        "NoSpaceAfterQuestionMark",
        &[
            "isNonJsxSameLineTokenContext",
            "isNonOptionalPropertyContext",
        ],
    ),
    (
        "NoSpaceBeforeDot",
        &[
            "isNonJsxSameLineTokenContext",
            "isNotPropertyAccessOnIntegerLiteral",
        ],
    ),
    ("NoSpaceAfterDot", &["isNonJsxSameLineTokenContext"]),
    (
        "NoSpaceBetweenImportParenInImportType",
        &["isNonJsxSameLineTokenContext", "isImportTypeContext"],
    ),
    (
        "NoSpaceAfterUnaryPrefixOperator",
        &["isNonJsxSameLineTokenContext", "isNotBinaryOpContext"],
    ),
    (
        "NoSpaceAfterUnaryPreincrementOperator",
        &["isNonJsxSameLineTokenContext"],
    ),
    (
        "NoSpaceAfterUnaryPredecrementOperator",
        &["isNonJsxSameLineTokenContext"],
    ),
    (
        "NoSpaceBeforeUnaryPostincrementOperator",
        &[
            "isNonJsxSameLineTokenContext",
            "isNotStatementConditionContext",
        ],
    ),
    (
        "NoSpaceBeforeUnaryPostdecrementOperator",
        &[
            "isNonJsxSameLineTokenContext",
            "isNotStatementConditionContext",
        ],
    ),
    (
        "SpaceAfterPostincrementWhenFollowedByAdd",
        &["isNonJsxSameLineTokenContext", "isBinaryOpContext"],
    ),
    (
        "SpaceAfterAddWhenFollowedByUnaryPlus",
        &["isNonJsxSameLineTokenContext", "isBinaryOpContext"],
    ),
    (
        "SpaceAfterAddWhenFollowedByPreincrement",
        &["isNonJsxSameLineTokenContext", "isBinaryOpContext"],
    ),
    (
        "SpaceAfterPostdecrementWhenFollowedBySubtract",
        &["isNonJsxSameLineTokenContext", "isBinaryOpContext"],
    ),
    (
        "SpaceAfterSubtractWhenFollowedByUnaryMinus",
        &["isNonJsxSameLineTokenContext", "isBinaryOpContext"],
    ),
    (
        "SpaceAfterSubtractWhenFollowedByPredecrement",
        &["isNonJsxSameLineTokenContext", "isBinaryOpContext"],
    ),
    ("NoSpaceAfterCloseBrace", &["isNonJsxSameLineTokenContext"]),
    (
        "NewLineBeforeCloseBraceInBlockContext",
        &["isMultilineBlockContext"],
    ),
    (
        "SpaceAfterCloseBrace",
        &["isNonJsxSameLineTokenContext", "isAfterCodeBlockContext"],
    ),
    (
        "SpaceBetweenCloseBraceAndElse",
        &["isNonJsxSameLineTokenContext"],
    ),
    (
        "SpaceBetweenCloseBraceAndWhile",
        &["isNonJsxSameLineTokenContext"],
    ),
    (
        "NoSpaceBetweenEmptyBraceBrackets",
        &["isNonJsxSameLineTokenContext", "isObjectContext"],
    ),
    (
        "SpaceAfterConditionalClosingParen",
        &["isControlDeclContext"],
    ),
    (
        "NoSpaceBetweenFunctionKeywordAndStar",
        &["isFunctionDeclarationOrFunctionExpressionContext"],
    ),
    (
        "SpaceAfterStarInGeneratorDeclaration",
        &["isFunctionDeclarationOrFunctionExpressionContext"],
    ),
    ("SpaceAfterFunctionInFuncDecl", &["isFunctionDeclContext"]),
    (
        "NewLineAfterOpenBraceInBlockContext",
        &["isMultilineBlockContext"],
    ),
    ("SpaceAfterGetSetInMember", &["isFunctionDeclContext"]),
    (
        "NoSpaceBetweenYieldKeywordAndStar",
        &[
            "isNonJsxSameLineTokenContext",
            "isYieldOrYieldStarWithOperand",
        ],
    ),
    (
        "SpaceBetweenYieldOrYieldStarAndOperand",
        &[
            "isNonJsxSameLineTokenContext",
            "isYieldOrYieldStarWithOperand",
        ],
    ),
    (
        "NoSpaceBetweenReturnAndSemicolon",
        &["isNonJsxSameLineTokenContext"],
    ),
    (
        "SpaceAfterCertainKeywords",
        &["isNonJsxSameLineTokenContext"],
    ),
    (
        "SpaceAfterLetConstInVariableDeclaration",
        &[
            "isNonJsxSameLineTokenContext",
            "isStartOfVariableDeclarationList",
        ],
    ),
    (
        "NoSpaceBeforeOpenParenInFuncCall",
        &[
            "isNonJsxSameLineTokenContext",
            "isFunctionCallOrNewContext",
            "isPreviousTokenNotComma",
        ],
    ),
    (
        "SpaceBeforeBinaryKeywordOperator",
        &["isNonJsxSameLineTokenContext", "isBinaryOpContext"],
    ),
    (
        "SpaceAfterBinaryKeywordOperator",
        &["isNonJsxSameLineTokenContext", "isBinaryOpContext"],
    ),
    (
        "SpaceAfterVoidOperator",
        &["isNonJsxSameLineTokenContext", "isVoidOpContext"],
    ),
    (
        "SpaceBetweenAsyncAndOpenParen",
        &["isArrowFunctionContext", "isNonJsxSameLineTokenContext"],
    ),
    (
        "SpaceBetweenAsyncAndFunctionKeyword",
        &["isNonJsxSameLineTokenContext"],
    ),
    (
        "NoSpaceBetweenTagAndTemplateString",
        &["isNonJsxSameLineTokenContext"],
    ),
    (
        "SpaceBeforeJsxAttribute",
        &[
            "isNextTokenParentJsxAttribute",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "SpaceBeforeSlashInJsxOpeningElement",
        &[
            "isJsxSelfClosingElementContext",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "NoSpaceBeforeGreaterThanTokenInJsxOpeningElement",
        &[
            "isJsxSelfClosingElementContext",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "NoSpaceBeforeEqualInJsxAttribute",
        &["isJsxAttributeContext", "isNonJsxSameLineTokenContext"],
    ),
    (
        "NoSpaceAfterEqualInJsxAttribute",
        &["isJsxAttributeContext", "isNonJsxSameLineTokenContext"],
    ),
    (
        "NoSpaceBeforeJsxNamespaceColon",
        &["isNextTokenParentJsxNamespacedName"],
    ),
    (
        "NoSpaceAfterJsxNamespaceColon",
        &["isNextTokenParentJsxNamespacedName"],
    ),
    (
        "NoSpaceAfterModuleImport",
        &["isNonJsxSameLineTokenContext"],
    ),
    (
        "SpaceAfterCertainTypeScriptKeywords",
        &["isNonJsxSameLineTokenContext"],
    ),
    (
        "SpaceBeforeCertainTypeScriptKeywords",
        &["isNonJsxSameLineTokenContext"],
    ),
    ("SpaceAfterModuleName", &["isModuleDeclContext"]),
    ("SpaceBeforeArrow", &["isNonJsxSameLineTokenContext"]),
    ("SpaceAfterArrow", &["isNonJsxSameLineTokenContext"]),
    ("NoSpaceAfterEllipsis", &["isNonJsxSameLineTokenContext"]),
    (
        "NoSpaceAfterOptionalParameters",
        &["isNonJsxSameLineTokenContext", "isNotBinaryOpContext"],
    ),
    (
        "NoSpaceBetweenEmptyInterfaceBraceBrackets",
        &["isNonJsxSameLineTokenContext", "isObjectTypeContext"],
    ),
    (
        "NoSpaceBeforeOpenAngularBracket",
        &[
            "isNonJsxSameLineTokenContext",
            "isTypeArgumentOrParameterOrAssertionContext",
        ],
    ),
    (
        "NoSpaceBetweenCloseParenAndAngularBracket",
        &[
            "isNonJsxSameLineTokenContext",
            "isTypeArgumentOrParameterOrAssertionContext",
        ],
    ),
    (
        "NoSpaceAfterOpenAngularBracket",
        &[
            "isNonJsxSameLineTokenContext",
            "isTypeArgumentOrParameterOrAssertionContext",
        ],
    ),
    (
        "NoSpaceBeforeCloseAngularBracket",
        &[
            "isNonJsxSameLineTokenContext",
            "isTypeArgumentOrParameterOrAssertionContext",
        ],
    ),
    (
        "NoSpaceAfterCloseAngularBracket",
        &[
            "isNonJsxSameLineTokenContext",
            "isTypeArgumentOrParameterOrAssertionContext",
            "isNotFunctionDeclContext",
            "isNonTypeAssertionContext",
        ],
    ),
    ("SpaceBeforeAt", &["isNonJsxSameLineTokenContext"]),
    ("NoSpaceAfterAt", &["isNonJsxSameLineTokenContext"]),
    (
        "SpaceAfterDecorator",
        &["isEndOfDecoratorContextOnSameLine"],
    ),
    (
        "NoSpaceBeforeNonNullAssertionOperator",
        &["isNonJsxSameLineTokenContext", "isNonNullAssertionContext"],
    ),
    (
        "NoSpaceAfterNewKeywordOnConstructorSignature",
        &[
            "isNonJsxSameLineTokenContext",
            "isConstructorSignatureContext",
        ],
    ),
    (
        "SpaceLessThanAndNonJSXTypeAnnotation",
        &["isNonJsxSameLineTokenContext"],
    ),
    (
        "SpaceAfterConstructor",
        &[
            "isOptionEnabled(insertSpaceAfterConstructorOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "NoSpaceAfterConstructor",
        &[
            "isOptionDisabledOrUndefined(insertSpaceAfterConstructorOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "SpaceAfterComma",
        &[
            "isOptionEnabled(insertSpaceAfterCommaDelimiterOption)",
            "isNonJsxSameLineTokenContext",
            "isNonJsxElementOrFragmentContext",
            "isNextTokenNotCloseBracket",
            "isNextTokenNotCloseParen",
        ],
    ),
    (
        "NoSpaceAfterComma",
        &[
            "isOptionDisabledOrUndefined(insertSpaceAfterCommaDelimiterOption)",
            "isNonJsxSameLineTokenContext",
            "isNonJsxElementOrFragmentContext",
        ],
    ),
    (
        "SpaceAfterAnonymousFunctionKeyword",
        &[
            "isOptionEnabled(insertSpaceAfterFunctionKeywordForAnonymousFunctionsOption)",
            "isFunctionDeclContext",
        ],
    ),
    (
        "NoSpaceAfterAnonymousFunctionKeyword",
        &[
            "isOptionDisabledOrUndefined(insertSpaceAfterFunctionKeywordForAnonymousFunctionsOption)",
            "isFunctionDeclContext",
        ],
    ),
    (
        "SpaceAfterKeywordInControl",
        &[
            "isOptionEnabled(insertSpaceAfterKeywordsInControlFlowStatementsOption)",
            "isControlDeclContext",
        ],
    ),
    (
        "NoSpaceAfterKeywordInControl",
        &[
            "isOptionDisabledOrUndefined(insertSpaceAfterKeywordsInControlFlowStatementsOption)",
            "isControlDeclContext",
        ],
    ),
    (
        "SpaceAfterOpenParen",
        &[
            "isOptionEnabled(insertSpaceAfterOpeningAndBeforeClosingNonemptyParenthesisOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "SpaceBeforeCloseParen",
        &[
            "isOptionEnabled(insertSpaceAfterOpeningAndBeforeClosingNonemptyParenthesisOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "SpaceBetweenOpenParens",
        &[
            "isOptionEnabled(insertSpaceAfterOpeningAndBeforeClosingNonemptyParenthesisOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    ("NoSpaceBetweenParens", &["isNonJsxSameLineTokenContext"]),
    (
        "NoSpaceAfterOpenParen",
        &[
            "isOptionDisabledOrUndefined(insertSpaceAfterOpeningAndBeforeClosingNonemptyParenthesisOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "NoSpaceBeforeCloseParen",
        &[
            "isOptionDisabledOrUndefined(insertSpaceAfterOpeningAndBeforeClosingNonemptyParenthesisOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "SpaceAfterOpenBracket",
        &[
            "isOptionEnabled(insertSpaceAfterOpeningAndBeforeClosingNonemptyBracketsOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "SpaceBeforeCloseBracket",
        &[
            "isOptionEnabled(insertSpaceAfterOpeningAndBeforeClosingNonemptyBracketsOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    ("NoSpaceBetweenBrackets", &["isNonJsxSameLineTokenContext"]),
    (
        "NoSpaceAfterOpenBracket",
        &[
            "isOptionDisabledOrUndefined(insertSpaceAfterOpeningAndBeforeClosingNonemptyBracketsOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "NoSpaceBeforeCloseBracket",
        &[
            "isOptionDisabledOrUndefined(insertSpaceAfterOpeningAndBeforeClosingNonemptyBracketsOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "SpaceAfterOpenBrace",
        &[
            "isOptionEnabledOrUndefined(insertSpaceAfterOpeningAndBeforeClosingNonemptyBracesOption)",
            "isBraceWrappedContext",
        ],
    ),
    (
        "SpaceBeforeCloseBrace",
        &[
            "isOptionEnabledOrUndefined(insertSpaceAfterOpeningAndBeforeClosingNonemptyBracesOption)",
            "isBraceWrappedContext",
        ],
    ),
    (
        "NoSpaceBetweenEmptyBraceBrackets",
        &["isNonJsxSameLineTokenContext", "isObjectContext"],
    ),
    (
        "NoSpaceAfterOpenBrace",
        &[
            "isOptionDisabled(insertSpaceAfterOpeningAndBeforeClosingNonemptyBracesOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "NoSpaceBeforeCloseBrace",
        &[
            "isOptionDisabled(insertSpaceAfterOpeningAndBeforeClosingNonemptyBracesOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "SpaceBetweenEmptyBraceBrackets",
        &["isOptionEnabled(insertSpaceAfterOpeningAndBeforeClosingEmptyBracesOption)"],
    ),
    (
        "NoSpaceBetweenEmptyBraceBrackets",
        &[
            "isOptionDisabled(insertSpaceAfterOpeningAndBeforeClosingEmptyBracesOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "SpaceAfterTemplateHeadAndMiddle",
        &[
            "isOptionEnabled(insertSpaceAfterOpeningAndBeforeClosingTemplateStringBracesOption)",
            "isNonJsxTextContext",
        ],
    ),
    (
        "SpaceBeforeTemplateMiddleAndTail",
        &[
            "isOptionEnabled(insertSpaceAfterOpeningAndBeforeClosingTemplateStringBracesOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "NoSpaceAfterTemplateHeadAndMiddle",
        &[
            "isOptionDisabledOrUndefined(insertSpaceAfterOpeningAndBeforeClosingTemplateStringBracesOption)",
            "isNonJsxTextContext",
        ],
    ),
    (
        "NoSpaceBeforeTemplateMiddleAndTail",
        &[
            "isOptionDisabledOrUndefined(insertSpaceAfterOpeningAndBeforeClosingTemplateStringBracesOption)",
            "isNonJsxSameLineTokenContext",
        ],
    ),
    (
        "SpaceAfterOpenBraceInJsxExpression",
        &[
            "isOptionEnabled(insertSpaceAfterOpeningAndBeforeClosingJsxExpressionBracesOption)",
            "isNonJsxSameLineTokenContext",
            "isJsxExpressionContext",
        ],
    ),
    (
        "SpaceBeforeCloseBraceInJsxExpression",
        &[
            "isOptionEnabled(insertSpaceAfterOpeningAndBeforeClosingJsxExpressionBracesOption)",
            "isNonJsxSameLineTokenContext",
            "isJsxExpressionContext",
        ],
    ),
    (
        "NoSpaceAfterOpenBraceInJsxExpression",
        &[
            "isOptionDisabledOrUndefined(insertSpaceAfterOpeningAndBeforeClosingJsxExpressionBracesOption)",
            "isNonJsxSameLineTokenContext",
            "isJsxExpressionContext",
        ],
    ),
    (
        "NoSpaceBeforeCloseBraceInJsxExpression",
        &[
            "isOptionDisabledOrUndefined(insertSpaceAfterOpeningAndBeforeClosingJsxExpressionBracesOption)",
            "isNonJsxSameLineTokenContext",
            "isJsxExpressionContext",
        ],
    ),
    (
        "SpaceAfterSemicolonInFor",
        &[
            "isOptionEnabled(insertSpaceAfterSemicolonInForStatementsOption)",
            "isNonJsxSameLineTokenContext",
            "isForContext",
        ],
    ),
    (
        "NoSpaceAfterSemicolonInFor",
        &[
            "isOptionDisabledOrUndefined(insertSpaceAfterSemicolonInForStatementsOption)",
            "isNonJsxSameLineTokenContext",
            "isForContext",
        ],
    ),
    (
        "SpaceBeforeBinaryOperator",
        &[
            "isOptionEnabled(insertSpaceBeforeAndAfterBinaryOperatorsOption)",
            "isNonJsxSameLineTokenContext",
            "isBinaryOpContext",
        ],
    ),
    (
        "SpaceAfterBinaryOperator",
        &[
            "isOptionEnabled(insertSpaceBeforeAndAfterBinaryOperatorsOption)",
            "isNonJsxSameLineTokenContext",
            "isBinaryOpContext",
        ],
    ),
    (
        "NoSpaceBeforeBinaryOperator",
        &[
            "isOptionDisabledOrUndefined(insertSpaceBeforeAndAfterBinaryOperatorsOption)",
            "isNonJsxSameLineTokenContext",
            "isBinaryOpContext",
        ],
    ),
    (
        "NoSpaceAfterBinaryOperator",
        &[
            "isOptionDisabledOrUndefined(insertSpaceBeforeAndAfterBinaryOperatorsOption)",
            "isNonJsxSameLineTokenContext",
            "isBinaryOpContext",
        ],
    ),
    (
        "SpaceBeforeOpenParenInFuncDecl",
        &[
            "isOptionEnabled(insertSpaceBeforeFunctionParenthesisOption)",
            "isNonJsxSameLineTokenContext",
            "isFunctionDeclContext",
        ],
    ),
    (
        "NoSpaceBeforeOpenParenInFuncDecl",
        &[
            "isOptionDisabledOrUndefined(insertSpaceBeforeFunctionParenthesisOption)",
            "isNonJsxSameLineTokenContext",
            "isFunctionDeclContext",
        ],
    ),
    (
        "NewLineBeforeOpenBraceInControl",
        &[
            "isOptionEnabled(placeOpenBraceOnNewLineForControlBlocksOption)",
            "isControlDeclContext",
            "isBeforeMultilineBlockContext",
        ],
    ),
    (
        "NewLineBeforeOpenBraceInFunction",
        &[
            "isOptionEnabled(placeOpenBraceOnNewLineForFunctionsOption)",
            "isFunctionDeclContext",
            "isBeforeMultilineBlockContext",
        ],
    ),
    (
        "NewLineBeforeOpenBraceInTypeScriptDeclWithBlock",
        &[
            "isOptionEnabled(placeOpenBraceOnNewLineForFunctionsOption)",
            "isTypeScriptDeclWithBlockContext",
            "isBeforeMultilineBlockContext",
        ],
    ),
    (
        "SpaceAfterTypeAssertion",
        &[
            "isOptionEnabled(insertSpaceAfterTypeAssertionOption)",
            "isNonJsxSameLineTokenContext",
            "isTypeAssertionContext",
        ],
    ),
    (
        "NoSpaceAfterTypeAssertion",
        &[
            "isOptionDisabledOrUndefined(insertSpaceAfterTypeAssertionOption)",
            "isNonJsxSameLineTokenContext",
            "isTypeAssertionContext",
        ],
    ),
    (
        "SpaceBeforeTypeAnnotation",
        &[
            "isOptionEnabled(insertSpaceBeforeTypeAnnotationOption)",
            "isNonJsxSameLineTokenContext",
            "isTypeAnnotationContext",
        ],
    ),
    (
        "NoSpaceBeforeTypeAnnotation",
        &[
            "isOptionDisabledOrUndefined(insertSpaceBeforeTypeAnnotationOption)",
            "isNonJsxSameLineTokenContext",
            "isTypeAnnotationContext",
        ],
    ),
    (
        "NoOptionalSemicolon",
        &[
            "optionEquals(semicolonOption, lsutil.SemicolonPreferenceRemove)",
            "isSemicolonDeletionContext",
        ],
    ),
    (
        "OptionalSemicolon",
        &[
            "optionEquals(semicolonOption, lsutil.SemicolonPreferenceInsert)",
            "isSemicolonInsertionContext",
        ],
    ),
    ("NoSpaceBeforeSemicolon", &["isNonJsxSameLineTokenContext"]),
    (
        "SpaceBeforeOpenBraceInControl",
        &[
            "isOptionDisabledOrUndefinedOrTokensOnSameLine(placeOpenBraceOnNewLineForControlBlocksOption)",
            "isControlDeclContext",
            "isNotFormatOnEnter",
            "isSameLineTokenOrBeforeBlockContext",
        ],
    ),
    (
        "SpaceBeforeOpenBraceInFunction",
        &[
            "isOptionDisabledOrUndefinedOrTokensOnSameLine(placeOpenBraceOnNewLineForFunctionsOption)",
            "isFunctionDeclContext",
            "isBeforeBlockContext",
            "isNotFormatOnEnter",
            "isSameLineTokenOrBeforeBlockContext",
        ],
    ),
    (
        "SpaceBeforeOpenBraceInTypeScriptDeclWithBlock",
        &[
            "isOptionDisabledOrUndefinedOrTokensOnSameLine(placeOpenBraceOnNewLineForFunctionsOption)",
            "isTypeScriptDeclWithBlockContext",
            "isNotFormatOnEnter",
            "isSameLineTokenOrBeforeBlockContext",
        ],
    ),
    ("NoSpaceBeforeComma", &["isNonJsxSameLineTokenContext"]),
    (
        "NoSpaceBeforeOpenBracket",
        &["isNonJsxSameLineTokenContext"],
    ),
    (
        "NoSpaceAfterCloseBracket",
        &[
            "isNonJsxSameLineTokenContext",
            "isNotBeforeBlockInFunctionDeclarationContext",
        ],
    ),
    ("SpaceAfterSemicolon", &["isNonJsxSameLineTokenContext"]),
    (
        "SpaceBetweenForAndAwaitKeyword",
        &["isNonJsxSameLineTokenContext"],
    ),
    (
        "SpaceBetweenDotDotDotAndTypeName",
        &["isNonJsxSameLineTokenContext"],
    ),
    (
        "SpaceBetweenStatements",
        &[
            "isNonJsxSameLineTokenContext",
            "isNonJsxElementOrFragmentContext",
            "isNotForContext",
        ],
    ),
    (
        "SpaceAfterTryCatchFinally",
        &["isNonJsxSameLineTokenContext"],
    ),
];
