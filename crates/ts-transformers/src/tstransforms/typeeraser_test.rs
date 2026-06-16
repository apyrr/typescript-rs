use ts_ast as ast;
use ts_core as core;
use ts_parser as parser;
use ts_printer as printer;

use super::typeeraser::{TypeEraserAction, type_eraser_action_for_kind};
use crate::{SourceFileTransformer, Transformer};

fn parse_typescript(text: &str) -> ast::SourceFile {
    parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: "/file.ts".to_string(),
            path: "/file.ts".to_string(),
            ..Default::default()
        },
        text.to_string(),
        core::ScriptKind::TS,
    )
}

fn emit_after_type_eraser(text: &str) -> String {
    let source_file = parse_typescript(text);
    assert!(
        source_file.diagnostics().is_empty(),
        "unexpected parse diagnostics"
    );

    let mut transformer = Transformer::default();
    transformer.new_source_file_transformer(
        SourceFileTransformer::TypeEraser {
            compiler_options: core::CompilerOptions::default(),
        },
        Some(printer::new_emit_context()),
    );
    let output = transformer.transform_source_file(&source_file);
    let emit_context = transformer
        .take_emit_context()
        .expect("transformer should retain emit context");
    let mut printer = printer::new_printer(
        printer::PrinterOptions::default(),
        printer::PrintHandlers::default(),
        Some(emit_context),
    );
    let text = printer.emit(&output.as_node(), Some(&output));
    text.strip_suffix('\n').unwrap_or(&text).to_string()
}

#[test]
fn type_eraser_go_case_actions_are_tracked() {
    let cases = [
        (ast::Kind::PublicKeyword, TypeEraserAction::Elide),
        (ast::Kind::InterfaceDeclaration, TypeEraserAction::Elide),
        (ast::Kind::TypeAliasDeclaration, TypeEraserAction::Elide),
        (
            ast::Kind::NamespaceExportDeclaration,
            TypeEraserAction::Elide,
        ),
        (
            ast::Kind::ExpressionWithTypeArguments,
            TypeEraserAction::StripTypeSyntax,
        ),
        (
            ast::Kind::PropertyDeclaration,
            TypeEraserAction::StripTypeSyntax,
        ),
        (ast::Kind::Constructor, TypeEraserAction::StripTypeSyntax),
        (
            ast::Kind::MethodDeclaration,
            TypeEraserAction::StripTypeSyntax,
        ),
        (ast::Kind::GetAccessor, TypeEraserAction::StripTypeSyntax),
        (ast::Kind::SetAccessor, TypeEraserAction::StripTypeSyntax),
        (ast::Kind::IndexSignature, TypeEraserAction::Elide),
        (
            ast::Kind::VariableDeclaration,
            TypeEraserAction::StripTypeSyntax,
        ),
        (
            ast::Kind::ClassDeclaration,
            TypeEraserAction::StripTypeSyntax,
        ),
        (
            ast::Kind::FunctionDeclaration,
            TypeEraserAction::StripTypeSyntax,
        ),
        (ast::Kind::ArrowFunction, TypeEraserAction::StripTypeSyntax),
        (ast::Kind::Parameter, TypeEraserAction::StripTypeSyntax),
        (ast::Kind::CallExpression, TypeEraserAction::StripTypeSyntax),
        (ast::Kind::NewExpression, TypeEraserAction::StripTypeSyntax),
        (
            ast::Kind::TaggedTemplateExpression,
            TypeEraserAction::StripTypeSyntax,
        ),
        (
            ast::Kind::NonNullExpression,
            TypeEraserAction::StripTypeSyntax,
        ),
        (
            ast::Kind::TypeAssertionExpression,
            TypeEraserAction::StripTypeSyntax,
        ),
        (ast::Kind::AsExpression, TypeEraserAction::StripTypeSyntax),
        (
            ast::Kind::SatisfiesExpression,
            TypeEraserAction::StripTypeSyntax,
        ),
        (
            ast::Kind::ImportDeclaration,
            TypeEraserAction::StripTypeSyntax,
        ),
        (
            ast::Kind::ExportDeclaration,
            TypeEraserAction::StripTypeSyntax,
        ),
    ];

    for (kind, expected) in cases {
        assert_eq!(type_eraser_action_for_kind(kind), expected, "{kind:?}");
    }
}

#[test]
fn printer_preserves_no_trailing_comma_after_type_erasure() {
    assert_eq!(emit_after_type_eraser("[a!]"), "[a];");
}

#[test]
fn printer_preserves_trailing_comma_after_type_erasure() {
    assert_eq!(emit_after_type_eraser("[a!,]"), "[a,];");
}

#[test]
fn type_eraser_visits_typescript_inside_blocks() {
    let output = emit_after_type_eraser("{ type Data = string | boolean; let obj: Data = true; }");
    assert_eq!(output, "{\n    let obj = true;\n}");
}

#[test]
fn printer_emits_partially_emitted_expression_after_type_erasure() {
    let output = emit_after_type_eraser(
        "return ((container.parent\n    .left as PropertyAccessExpression)\n    .expression as PropertyAccessExpression)\n    .expression;",
    );
    assert_eq!(
        output,
        "return container.parent\n    .left\n    .expression\n    .expression;"
    );
}
