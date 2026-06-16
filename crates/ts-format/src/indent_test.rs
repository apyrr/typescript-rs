use ts_ast as ast;
use ts_core as core;
use ts_parser as parser;

use std::ops::ControlFlow;

use super::*;

#[test]
fn test_get_containing_list_named_imports() {
    let text = "import type {\n    AAA,\n    BBB,\n} from \"./bar\";";

    let source_file = parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: "/test.ts".to_owned(),
            path: "/test.ts".to_owned(),
            external_module_indicator_options: Default::default(),
        },
        text.to_owned(),
        core::ScriptKind::TS,
    );

    // Find ImportSpecifier nodes (AAA and BBB)
    let mut import_specifiers = Vec::new();
    for_each_descendant_of_kind(
        source_file.store(),
        source_file.as_node(),
        ast::Kind::ImportSpecifier,
        &mut |node| {
            import_specifiers.push(node);
        },
    );

    assert_eq!(
        import_specifiers.len(),
        2,
        "Expected 2 import specifiers, got {}",
        import_specifiers.len()
    );

    // Test GetContainingList for each import specifier
    for specifier in import_specifiers {
        let list = get_containing_list(&specifier, &source_file);
        assert!(
            list.is_some(),
            "GetContainingList should return non-nil for import specifier"
        );
        let list = list.unwrap();
        assert_eq!(
            list.len(),
            2,
            "Expected list with 2 elements, got {}",
            list.len()
        );
    }
}

fn for_each_descendant_of_kind(
    store: &ast::AstStore,
    node: ast::Node,
    kind: ast::Kind,
    action: &mut impl FnMut(ast::Node),
) {
    let _ = store.for_each_present_child(node, |child| {
        if store.kind(child) == kind {
            action(child);
        }
        for_each_descendant_of_kind(store, child, kind, action);
        ControlFlow::Continue(())
    });
}
