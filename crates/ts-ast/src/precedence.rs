use crate::*;

pub type OperatorPrecedence = i32;

// Expression:
//     AssignmentExpression
//     Expression `,` AssignmentExpression
pub const OPERATOR_PRECEDENCE_COMMA: OperatorPrecedence = 0;
// NOTE: `Spread` is higher than `Comma` due to how it is parsed in |ElementList|
// SpreadElement:
//     `...` AssignmentExpression
pub const OPERATOR_PRECEDENCE_SPREAD: OperatorPrecedence = 1;
// AssignmentExpression:
//     ConditionalExpression
//     YieldExpression
//     ArrowFunction
//     AsyncArrowFunction
//     LeftHandSideExpression `=` AssignmentExpression
//     LeftHandSideExpression AssignmentOperator AssignmentExpression
//
// NOTE: AssignmentExpression is broken down into several precedences due to the requirements
//       of the parenthesizer rules.
// AssignmentExpression: YieldExpression
// YieldExpression:
//     `yield`
//     `yield` AssignmentExpression
//     `yield` `*` AssignmentExpression
pub const OPERATOR_PRECEDENCE_YIELD: OperatorPrecedence = 2;
// AssignmentExpression: LeftHandSideExpression `=` AssignmentExpression
// AssignmentExpression: LeftHandSideExpression AssignmentOperator AssignmentExpression
// AssignmentOperator: one of
//     `*=` `/=` `%=` `+=` `-=` `<<=` `>>=` `>>>=` `&=` `^=` `|=` `**=`
pub const OPERATOR_PRECEDENCE_ASSIGNMENT: OperatorPrecedence = 3;
// NOTE: `Conditional` is considered higher than `Assignment` here, but in reality they have
//       the same precedence.
// AssignmentExpression: ConditionalExpression
// ConditionalExpression:
//     ShortCircuitExpression
//     ShortCircuitExpression `?` AssignmentExpression `:` AssignmentExpression
pub const OPERATOR_PRECEDENCE_CONDITIONAL: OperatorPrecedence = 4;
// LogicalORExpression:
//     LogicalANDExpression
//     LogicalORExpression `||` LogicalANDExpression
pub const OPERATOR_PRECEDENCE_LOGICAL_OR: OperatorPrecedence = 5;
// LogicalANDExpression:
//     BitwiseORExpression
//     LogicalANDExprerssion `&&` BitwiseORExpression
pub const OPERATOR_PRECEDENCE_LOGICAL_AND: OperatorPrecedence = 6;
// BitwiseORExpression:
//     BitwiseXORExpression
//     BitwiseORExpression `|` BitwiseXORExpression
pub const OPERATOR_PRECEDENCE_BITWISE_OR: OperatorPrecedence = 7;
// BitwiseXORExpression:
//     BitwiseANDExpression
//     BitwiseXORExpression `^` BitwiseANDExpression
pub const OPERATOR_PRECEDENCE_BITWISE_XOR: OperatorPrecedence = 8;
// BitwiseANDExpression:
//     EqualityExpression
//     BitwiseANDExpression `&` EqualityExpression
pub const OPERATOR_PRECEDENCE_BITWISE_AND: OperatorPrecedence = 9;
// EqualityExpression:
//     RelationalExpression
//     EqualityExpression `==` RelationalExpression
//     EqualityExpression `!=` RelationalExpression
//     EqualityExpression `===` RelationalExpression
//     EqualityExpression `!==` RelationalExpression
pub const OPERATOR_PRECEDENCE_EQUALITY: OperatorPrecedence = 10;
// RelationalExpression:
//     ShiftExpression
//     RelationalExpression `<` ShiftExpression
//     RelationalExpression `>` ShiftExpression
//     RelationalExpression `<=` ShiftExpression
//     RelationalExpression `>=` ShiftExpression
//     RelationalExpression `instanceof` ShiftExpression
//     RelationalExpression `in` ShiftExpression
//     [+TypeScript] RelationalExpression `as` Type
pub const OPERATOR_PRECEDENCE_RELATIONAL: OperatorPrecedence = 11;
// ShiftExpression:
//     AdditiveExpression
//     ShiftExpression `<<` AdditiveExpression
//     ShiftExpression `>>` AdditiveExpression
//     ShiftExpression `>>>` AdditiveExpression
pub const OPERATOR_PRECEDENCE_SHIFT: OperatorPrecedence = 12;
// AdditiveExpression:
//     MultiplicativeExpression
//     AdditiveExpression `+` MultiplicativeExpression
//     AdditiveExpression `-` MultiplicativeExpression
pub const OPERATOR_PRECEDENCE_ADDITIVE: OperatorPrecedence = 13;
// MultiplicativeExpression:
//     ExponentiationExpression
//     MultiplicativeExpression MultiplicativeOperator ExponentiationExpression
// MultiplicativeOperator: one of `*`, `/`, `%`
pub const OPERATOR_PRECEDENCE_MULTIPLICATIVE: OperatorPrecedence = 14;
// ExponentiationExpression:
//     UnaryExpression
//     UpdateExpression `**` ExponentiationExpression
pub const OPERATOR_PRECEDENCE_EXPONENTIATION: OperatorPrecedence = 15;
// UnaryExpression:
//     UpdateExpression
//     `delete` UnaryExpression
//     `void` UnaryExpression
//     `typeof` UnaryExpression
//     `+` UnaryExpression
//     `-` UnaryExpression
//     `~` UnaryExpression
//     `!` UnaryExpression
//     AwaitExpression
// UpdateExpression:            // TODO: Do we need to investigate the precedence here?
//     `++` UnaryExpression
//     `--` UnaryExpression
pub const OPERATOR_PRECEDENCE_UNARY: OperatorPrecedence = 16;
// UpdateExpression:
//     LeftHandSideExpression
//     LeftHandSideExpression `++`
//     LeftHandSideExpression `--`
pub const OPERATOR_PRECEDENCE_UPDATE: OperatorPrecedence = 17;
// LeftHandSideExpression:
//     NewExpression
// NewExpression:
//     MemberExpression
//     `new` NewExpression
pub const OPERATOR_PRECEDENCE_LEFT_HAND_SIDE: OperatorPrecedence = 18;
// LeftHandSideExpression:
//     OptionalExpression
// OptionalExpression:
//     MemberExpression OptionalChain
//     CallExpression OptionalChain
//     OptionalExpression OptionalChain
pub const OPERATOR_PRECEDENCE_OPTIONAL_CHAIN: OperatorPrecedence = 19;
// LeftHandSideExpression:
//     CallExpression
// CallExpression:
//     CoverCallExpressionAndAsyncArrowHead
//     SuperCall
//     ImportCall
//     CallExpression Arguments
//     CallExpression `[` Expression `]`
//     CallExpression `.` IdentifierName
//     CallExpression TemplateLiteral
// MemberExpression:
//     PrimaryExpression
//     MemberExpression `[` Expression `]`
//     MemberExpression `.` IdentifierName
//     MemberExpression TemplateLiteral
//     SuperProperty
//     MetaProperty
//     `new` MemberExpression Arguments
pub const OPERATOR_PRECEDENCE_MEMBER: OperatorPrecedence = 20;
// TODO: JSXElement?
// PrimaryExpression:
//     `this`
//     IdentifierReference
//     Literal
//     ArrayLiteral
//     ObjectLiteral
//     FunctionExpression
//     ClassExpression
//     GeneratorExpression
//     AsyncFunctionExpression
//     AsyncGeneratorExpression
//     RegularExpressionLiteral
//     TemplateLiteral
pub const OPERATOR_PRECEDENCE_PRIMARY: OperatorPrecedence = 21;
// PrimaryExpression:
//     CoverParenthesizedExpressionAndArrowParameterList
pub const OPERATOR_PRECEDENCE_PARENTHESES: OperatorPrecedence = 22;
pub const OPERATOR_PRECEDENCE_LOWEST: OperatorPrecedence = OPERATOR_PRECEDENCE_COMMA;
pub const OPERATOR_PRECEDENCE_HIGHEST: OperatorPrecedence = OPERATOR_PRECEDENCE_PARENTHESES;
pub const OPERATOR_PRECEDENCE_DISALLOW_COMMA: OperatorPrecedence = OPERATOR_PRECEDENCE_YIELD;
// ShortCircuitExpression:
//     LogicalORExpression
//     CoalesceExpression
// CoalesceExpression:
//     CoalesceExpressionHead `??` BitwiseORExpression
// CoalesceExpressionHead:
//     CoalesceExpression
//     BitwiseORExpression
pub const OPERATOR_PRECEDENCE_COALESCE: OperatorPrecedence = OPERATOR_PRECEDENCE_LOGICAL_OR;
// -1 is lower than all other precedences. Returning it will cause binary expression
// parsing to stop.
pub const OPERATOR_PRECEDENCE_INVALID: OperatorPrecedence = -1;

fn get_operator(store: &AstStore, expression: &Node) -> Kind {
    match store.kind(*expression) {
        Kind::BinaryExpression => {
            let operator = store.as_binary_expression(*expression).operator_token;
            store.kind(store.node_from_id(operator))
        }
        Kind::PrefixUnaryExpression => store.as_prefix_unary_expression(*expression).operator,
        Kind::PostfixUnaryExpression => store.as_postfix_unary_expression(*expression).operator,
        _ => store.kind(*expression),
    }
}

// Gets the precedence of an expression
pub fn get_expression_precedence(store: &AstStore, expression: &Node) -> OperatorPrecedence {
    let operator = get_operator(store, expression);
    let mut flags = OPERATOR_PRECEDENCE_FLAGS_NONE;
    let kind = store.kind(*expression);
    if kind == Kind::NewExpression && store.arguments(*expression).is_none() {
        flags = OPERATOR_PRECEDENCE_FLAGS_NEW_WITHOUT_ARGUMENTS;
    } else if store.flags(*expression).contains(NodeFlags::OPTIONAL_CHAIN) {
        flags = OPERATOR_PRECEDENCE_FLAGS_OPTIONAL_CHAIN;
    }
    get_operator_precedence(kind, operator, flags)
}

pub type OperatorPrecedenceFlags = i32;

pub const OPERATOR_PRECEDENCE_FLAGS_NONE: OperatorPrecedenceFlags = 0;
pub const OPERATOR_PRECEDENCE_FLAGS_NEW_WITHOUT_ARGUMENTS: OperatorPrecedenceFlags = 1 << 0;
pub const OPERATOR_PRECEDENCE_FLAGS_OPTIONAL_CHAIN: OperatorPrecedenceFlags = 1 << 1;

// Gets the precedence of an operator
pub fn get_operator_precedence(
    node_kind: Kind,
    operator_kind: Kind,
    flags: OperatorPrecedenceFlags,
) -> OperatorPrecedence {
    match node_kind {
        Kind::SpreadElement => OPERATOR_PRECEDENCE_SPREAD,
        Kind::YieldExpression => OPERATOR_PRECEDENCE_YIELD,
        // !!! By necessity, this differs from the old compiler to better align with ParenthesizerRules. consider backporting
        Kind::ArrowFunction => OPERATOR_PRECEDENCE_ASSIGNMENT,
        Kind::ConditionalExpression => OPERATOR_PRECEDENCE_CONDITIONAL,
        Kind::BinaryExpression => match operator_kind {
            Kind::CommaToken => OPERATOR_PRECEDENCE_COMMA,

            Kind::EqualsToken
            | Kind::PlusEqualsToken
            | Kind::MinusEqualsToken
            | Kind::AsteriskAsteriskEqualsToken
            | Kind::AsteriskEqualsToken
            | Kind::SlashEqualsToken
            | Kind::PercentEqualsToken
            | Kind::LessThanLessThanEqualsToken
            | Kind::GreaterThanGreaterThanEqualsToken
            | Kind::GreaterThanGreaterThanGreaterThanEqualsToken
            | Kind::AmpersandEqualsToken
            | Kind::CaretEqualsToken
            | Kind::BarEqualsToken
            | Kind::BarBarEqualsToken
            | Kind::AmpersandAmpersandEqualsToken
            | Kind::QuestionQuestionEqualsToken => OPERATOR_PRECEDENCE_ASSIGNMENT,

            _ => get_binary_operator_precedence(operator_kind),
        },
        // TODO: Should prefix `++` and `--` be moved to the `Update` precedence?
        Kind::TypeAssertionExpression
        | Kind::NonNullExpression
        | Kind::PrefixUnaryExpression
        | Kind::TypeOfExpression
        | Kind::VoidExpression
        | Kind::DeleteExpression
        | Kind::AwaitExpression => OPERATOR_PRECEDENCE_UNARY,

        Kind::PostfixUnaryExpression => OPERATOR_PRECEDENCE_UPDATE,

        // !!! By necessity, this differs from the old compiler to better align with ParenthesizerRules. consider backporting
        Kind::PropertyAccessExpression | Kind::ElementAccessExpression => {
            if flags & OPERATOR_PRECEDENCE_FLAGS_OPTIONAL_CHAIN != 0 {
                return OPERATOR_PRECEDENCE_OPTIONAL_CHAIN;
            }
            OPERATOR_PRECEDENCE_MEMBER
        }

        Kind::CallExpression => {
            if flags & OPERATOR_PRECEDENCE_FLAGS_OPTIONAL_CHAIN != 0 {
                return OPERATOR_PRECEDENCE_OPTIONAL_CHAIN;
            }
            OPERATOR_PRECEDENCE_MEMBER
        }

        // !!! By necessity, this differs from the old compiler to better align with ParenthesizerRules. consider backporting
        Kind::NewExpression => {
            if flags & OPERATOR_PRECEDENCE_FLAGS_NEW_WITHOUT_ARGUMENTS != 0 {
                return OPERATOR_PRECEDENCE_LEFT_HAND_SIDE;
            }
            OPERATOR_PRECEDENCE_MEMBER
        }

        // !!! By necessity, this differs from the old compiler to better align with ParenthesizerRules. consider backporting
        Kind::TaggedTemplateExpression | Kind::MetaProperty | Kind::ExpressionWithTypeArguments => {
            OPERATOR_PRECEDENCE_MEMBER
        }

        Kind::AsExpression | Kind::SatisfiesExpression => OPERATOR_PRECEDENCE_RELATIONAL,

        Kind::ThisKeyword
        | Kind::SuperKeyword
        | Kind::ImportKeyword
        | Kind::Identifier
        | Kind::PrivateIdentifier
        | Kind::NullKeyword
        | Kind::TrueKeyword
        | Kind::FalseKeyword
        | Kind::NumericLiteral
        | Kind::BigIntLiteral
        | Kind::StringLiteral
        | Kind::ArrayLiteralExpression
        | Kind::ObjectLiteralExpression
        | Kind::FunctionExpression
        | Kind::ClassExpression
        | Kind::RegularExpressionLiteral
        | Kind::NoSubstitutionTemplateLiteral
        | Kind::TemplateExpression
        | Kind::OmittedExpression
        | Kind::JsxElement
        | Kind::JsxSelfClosingElement
        | Kind::JsxFragment
        | Kind::MissingDeclaration => OPERATOR_PRECEDENCE_PRIMARY,

        // !!! By necessity, this differs from the old compiler to support emit. consider backporting
        Kind::ParenthesizedExpression => OPERATOR_PRECEDENCE_PARENTHESES,

        _ => OPERATOR_PRECEDENCE_INVALID,
    }
}

// Gets the precedence of a binary operator
pub fn get_binary_operator_precedence(operator_kind: Kind) -> OperatorPrecedence {
    match operator_kind {
        Kind::QuestionQuestionToken => OPERATOR_PRECEDENCE_COALESCE,
        Kind::BarBarToken => OPERATOR_PRECEDENCE_LOGICAL_OR,
        Kind::AmpersandAmpersandToken => OPERATOR_PRECEDENCE_LOGICAL_AND,
        Kind::BarToken => OPERATOR_PRECEDENCE_BITWISE_OR,
        Kind::CaretToken => OPERATOR_PRECEDENCE_BITWISE_XOR,
        Kind::AmpersandToken => OPERATOR_PRECEDENCE_BITWISE_AND,
        Kind::EqualsEqualsToken
        | Kind::ExclamationEqualsToken
        | Kind::EqualsEqualsEqualsToken
        | Kind::ExclamationEqualsEqualsToken => OPERATOR_PRECEDENCE_EQUALITY,
        Kind::LessThanToken
        | Kind::GreaterThanToken
        | Kind::LessThanEqualsToken
        | Kind::GreaterThanEqualsToken
        | Kind::InstanceOfKeyword
        | Kind::InKeyword
        | Kind::AsKeyword
        | Kind::SatisfiesKeyword => OPERATOR_PRECEDENCE_RELATIONAL,
        Kind::LessThanLessThanToken
        | Kind::GreaterThanGreaterThanToken
        | Kind::GreaterThanGreaterThanGreaterThanToken => OPERATOR_PRECEDENCE_SHIFT,
        Kind::PlusToken | Kind::MinusToken => OPERATOR_PRECEDENCE_ADDITIVE,
        Kind::AsteriskToken | Kind::SlashToken | Kind::PercentToken => {
            OPERATOR_PRECEDENCE_MULTIPLICATIVE
        }
        Kind::AsteriskAsteriskToken => OPERATOR_PRECEDENCE_EXPONENTIATION,
        _ => {
            // -1 is lower than all other precedences.  Returning it will cause binary expression
            // parsing to stop.
            OPERATOR_PRECEDENCE_INVALID
        }
    }
}

// Gets the leftmost expression of an expression, e.g. `a` in `a.b`, `a[b]`, `a++`, `a+b`, `a?b:c`, `a as B`, etc.
pub fn get_leftmost_expression(
    store: &AstStore,
    node: &Node,
    stop_at_call_expressions: bool,
) -> Node {
    let mut node = *node;
    loop {
        match store.kind(node) {
            Kind::PostfixUnaryExpression => {
                let next = store.node_from_id(store.as_postfix_unary_expression(node).operand);
                node = next;
                continue;
            }
            Kind::BinaryExpression => {
                let next = store.node_from_id(store.as_binary_expression(node).left);
                node = next;
                continue;
            }
            Kind::ConditionalExpression => {
                let next = store.node_from_id(store.as_conditional_expression(node).condition);
                node = next;
                continue;
            }
            Kind::TaggedTemplateExpression => {
                let next = store.node_from_id(store.as_tagged_template_expression(node).tag);
                node = next;
                continue;
            }
            Kind::CallExpression => {
                if stop_at_call_expressions {
                    return node;
                }
                node = store
                    .expression(node)
                    .expect("CallExpression should have an expression");
                continue;
            }
            Kind::AsExpression
            | Kind::ElementAccessExpression
            | Kind::PropertyAccessExpression
            | Kind::NonNullExpression
            | Kind::PartiallyEmittedExpression
            | Kind::SatisfiesExpression => {
                node = store
                    .expression(node)
                    .expect("expression node should have an expression");
                continue;
            }
            _ => return node,
        }
    }
}

pub type TypePrecedence = i32;

// Conditional precedence (lowest)
//
//   Type[Extends]:
//       ConditionalTypeNode[?Extends]
//
//   ConditionalTypeNode[Extends]:
//       [~Extends] UnionTypeNode `extends` Type[+Extends] `?` Type[~Extends] `:` Type[~Extends]
//
pub const TYPE_PRECEDENCE_CONDITIONAL: TypePrecedence = 0;

// Function precedence
//
//   Type[Extends]:
//       ConditionalTypeNode[?Extends]
//       FunctionTypeNode[?Extends]
//       ConstructorTypeNode[?Extends]
//
//   ConditionalTypeNode[Extends]:
//       UnionTypeNode
//
//   FunctionTypeNode[Extends]:
//       TypeParameters? ArrowParameters `=>` Type[?Extends]
//
//   ConstructorTypeNode[Extends]:
//       `abstract`? TypeParameters? ArrowParameters `=>` Type[?Extends]
//
pub const TYPE_PRECEDENCE_FUNCTION: TypePrecedence = 2;

// Union precedence
//
//   UnionTypeNode:
//       `|`? UnionTypeNoBar
//
//   UnionTypeNoBar:
//       IntersectionTypeNode
//       UnionTypeNoBar `|` IntersectionTypeNode
//
pub const TYPE_PRECEDENCE_UNION: TypePrecedence = 3;

// Intersection precedence
//
//   IntersectionTypeNode:
//       `&`? IntersectionTypeNoAmpersand
//
//   IntersectionTypeNoAmpersand:
//       TypeOperatorNode
//       IntersectionTypeNoAmpersand `&` TypeOperatorNode
//
pub const TYPE_PRECEDENCE_INTERSECTION: TypePrecedence = 4;

// TypeOperatorNode precedence
//
//   TypeOperatorNode:
//     PostfixType
//     InferTypeNode
//     `keyof` TypeOperatorNode
//     `unique` TypeOperatorNode
//     `readonly` PostfixType
//
//   InferTypeNode:
//     `infer` BindingIdentifier
//     `infer` BindingIdentifier `extends` Type[+Extends]
//
pub const TYPE_PRECEDENCE_TYPE_OPERATOR: TypePrecedence = 5;

// Postfix precedence
//
//   PostfixType:
//       NonArrayType
//       OptionalTypeNode
//       ArrayTypeNode
//       IndexedAccessTypeNode
//
//   OptionalTypeNode:
//       PostfixType `?`
//
//   ArrayTypeNode:
//       PostfixType `[` `]`
//
//   IndexedAccessTypeNode:
//       PostfixType `[` Type[~Extends] `]`
//
pub const TYPE_PRECEDENCE_POSTFIX: TypePrecedence = 6;

// NonArray precedence (highest)
//
//   NonArrayType:
//       KeywordType
//       LiteralTypeNode
//       ThisTypeNode
//       ImportType
//       TypeQueryNode
//       MappedTypeNode
//       TypeLiteralNode
//       TupleTypeNode
//       ParenthesizedTypeNode
//       TypePredicateNode
//       TypeReferenceNode
//       TemplateType
//
//   KeywordType: one of
//       `any`       `unknown` `string`    `number` `bigint`
//       `symbol`    `boolean` `undefined` `never`  `object`
//       `intrinsic` `void`
//
//   LiteralTypeNode:
//       StringLiteral
//       NoSubstitutionTemplateLiteral
//       NumericLiteral
//       BigIntLiteral
//       `-` NumericLiteral
//       `-` BigIntLiteral
//       `true`
//       `false`
//       `null`
//
//   ThisTypeNode:
//       `this`
//
//   ImportType:
//       `typeof`? `import` `(` Type[~Extends] `,`? `)` ImportTypeQualifier? TypeArguments?
//       `typeof`? `import` `(` Type[~Extends] `,` ImportTypeAttributes `,`? `)` ImportTypeQualifier? TypeArguments?
//
//   ImportTypeQualifier:
//       `.` EntityName
//
//   ImportTypeAttributes:
//       `{` `with` `:` ImportAttributes `,`? `}`
//
//   TypeQueryNode:
//
//   MappedTypeNode:
//       `{` MappedTypePrefix? MappedTypePropertyName MappedTypeSuffix? `:` Type[~Extends] `;` `}`
//
//   MappedTypePrefix:
//       `readonly`
//       `+` `readonly`
//       `-` `readonly`
//
//   MappedTypePropertyName:
//       `[` BindingIdentifier `in` Type[~Extends] `]`
//       `[` BindingIdentifier `in` Type[~Extends] `as` Type[~Extends] `]`
//
//   MappedTypeSuffix:
//       `?`
//       `+` `?`
//       `-` `?`
//
//   TypeLiteralNode:
//       `{` TypeElementList `}`
//
//   TypeElementList:
//       [empty]
//       TypeElementList TypeElement
//
//   TypeElement:
//       PropertySignatureDeclaration
//       MethodSignatureDeclaration
//       IndexSignatureDeclaration
//       CallSignatureDeclaration
//       ConstructSignatureDeclaration
//
//   PropertySignatureDeclaration:
//       PropertyName `?`? TypeAnnotation? `;`
//
//   MethodSignatureDeclaration:
//       PropertyName `?`? TypeParameters? `(` FormalParameterList `)` TypeAnnotation? `;`
//       `get` PropertyName TypeParameters? `(` FormalParameterList `)` TypeAnnotation? `;` // GetAccessorDeclaration
//       `set` PropertyName TypeParameters? `(` FormalParameterList `)` TypeAnnotation? `;` // SetAccessorDeclaration
//
//   IndexSignatureDeclaration:
//       `[` IdentifierName`]` TypeAnnotation `;`
//
//   CallSignatureDeclaration:
//       TypeParameters? `(` FormalParameterList `)` TypeAnnotation? `;`
//
//   ConstructSignatureDeclaration:
//       `new` TypeParameters? `(` FormalParameterList `)` TypeAnnotation? `;`
//
//   TupleTypeNode:
//       `[` `]`
//       `[` NamedTupleElementTypes `,`? `]`
//       `[` TupleElementTypes `,`? `]`
//
//   NamedTupleElementTypes:
//       NamedTupleMember
//       NamedTupleElementTypes `,` NamedTupleMember
//
//   NamedTupleMember:
//       IdentifierName `?`? `:` Type[~Extends]
//       `...` IdentifierName `:` Type[~Extends]
//
//   TupleElementTypes:
//       TupleElementType
//       TupleElementTypes `,` TupleElementType
//
//   TupleElementType:
//       Type[~Extends]
//       OptionalTypeNode
//       RestTypeNode
//
//   RestTypeNode:
//       `...` Type[~Extends]
//
//   ParenthesizedTypeNode:
//       `(` Type[~Extends] `)`
//
//   TypePredicateNode:
//       `asserts`? TypePredicateParameterName
//       `asserts`? TypePredicateParameterName `is` Type[~Extends]
//
//   TypePredicateParameterName:
//       `this`
//       IdentifierReference
//
//   TypeReferenceNode:
//       EntityName TypeArguments?
//
//   TemplateType:
//       TemplateHead Type[~Extends] TemplateTypeSpans
//
//   TemplateTypeSpans:
//       TemplateTail
//       TemplateTypeMiddleList TemplateTail
//
//   TemplateTypeMiddleList:
//       TemplateMiddle Type[~Extends]
//       TemplateTypeMiddleList TemplateMiddle Type[~Extends]
//
//   TypeArguments:
//       `<` TypeArgumentList `,`? `>`
//
//   TypeArgumentList:
//       Type[~Extends]
//       TypeArgumentList `,` Type[~Extends]
//
pub const TYPE_PRECEDENCE_NON_ARRAY: TypePrecedence = 7;

pub const TYPE_PRECEDENCE_LOWEST: TypePrecedence = TYPE_PRECEDENCE_CONDITIONAL;
pub const TYPE_PRECEDENCE_HIGHEST: TypePrecedence = TYPE_PRECEDENCE_NON_ARRAY;

// Gets the precedence of a TypeNode
pub fn get_type_node_precedence(store: &AstStore, n: &Node) -> TypePrecedence {
    match store.kind(*n) {
        Kind::ConditionalType => TYPE_PRECEDENCE_CONDITIONAL,
        Kind::FunctionType | Kind::ConstructorType => TYPE_PRECEDENCE_FUNCTION,
        Kind::UnionType => TYPE_PRECEDENCE_UNION,
        Kind::IntersectionType => TYPE_PRECEDENCE_INTERSECTION,
        Kind::TypeOperator => TYPE_PRECEDENCE_TYPE_OPERATOR,
        Kind::InferType => {
            let type_parameter = store.node_from_id(store.as_infer_type_node(*n).type_parameter);
            if store
                .as_type_parameter_declaration(type_parameter)
                .constraint
                .is_some()
            {
                // `infer T extends U` must be treated as FunctionTypeNode precedence as the `extends` clause eagerly consumes
                // TypeNode
                return TYPE_PRECEDENCE_FUNCTION;
            }
            TYPE_PRECEDENCE_TYPE_OPERATOR
        }
        Kind::IndexedAccessType | Kind::ArrayType | Kind::OptionalType => {
            TYPE_PRECEDENCE_POSTFIX
        }
        Kind::TypeQuery => {
            // TypeQueryNode is actually a NonArrayType, but we treat it as TypeOperatorNode
            // precedence so that it is parenthesized when used in a PostfixType
            // context (e.g., `(typeof C)[]` instead of `typeof C[]`)
            TYPE_PRECEDENCE_TYPE_OPERATOR
        }
        Kind::AnyKeyword
        | Kind::UnknownKeyword
        | Kind::StringKeyword
        | Kind::NumberKeyword
        | Kind::BigIntKeyword
        | Kind::SymbolKeyword
        | Kind::BooleanKeyword
        | Kind::UndefinedKeyword
        | Kind::NeverKeyword
        | Kind::ObjectKeyword
        | Kind::IntrinsicKeyword
        | Kind::VoidKeyword
        | Kind::LiteralType
        | Kind::TypePredicate
        | Kind::TypeReference
        | Kind::TypeLiteral
        | Kind::TupleType
        | Kind::RestType
        | Kind::ParenthesizedType
        | Kind::ThisType
        | Kind::MappedType
        | Kind::NamedTupleMember
        | Kind::TemplateLiteralType
        | Kind::ImportType
        // These occur in pseudo-types like `f<T>.C`, where `f` is a generic function and `C` is a local type
        | Kind::PropertyAccessExpression
        | Kind::ExpressionWithTypeArguments => TYPE_PRECEDENCE_NON_ARRAY,
        _ => panic!("unhandled TypeNode: {}", store.kind(*n)),
    }
}
