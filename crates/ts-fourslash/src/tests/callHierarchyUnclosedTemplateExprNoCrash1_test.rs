use crate::{new_fourslash, TestingT};

pub fn test_call_hierarchy_unclosed_template_expr_no_crash1(t: &mut TestingT) {
    // Regression test for a crash in prepareCallHierarchy caused by parser error
    // recovery: when a template expression is truncated mid-call (e.g. `${format`
    // without closing `)`), the parser misinterprets the `class` keyword in
    // subsequent HTML template literals as a TypeScript class declaration.
    // The resulting anonymous ClassDeclaration (no name, no `default` modifier)
    // previously caused a "Expected call hierarchy declaration to have a reference
    // node" assertion failure.
    let content = "// @Filename: /main.ts\n\
function updateBadge() {\n\
    const header = `<div class=\"sub\">${format`;\n\
    const badge = `<div /*1*/class=\"badge\">`;\n\
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_baseline_call_hierarchy(t);
    done();
}

