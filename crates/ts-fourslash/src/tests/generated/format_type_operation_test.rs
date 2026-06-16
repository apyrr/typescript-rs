#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_type_operation() {
    let mut t = TestingT;
    run_test_format_type_operation(&mut t);
}

fn run_test_format_type_operation(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"type   Union = number  |  {}/*formatBarOperator*/
/*indent*/
|string/*autoformat*/
type  Intersection   =   Foo    &    Bar;/*formatAmpersandOperator*/
type Complexed =
    Foo&
    Bar|/*unionTypeNoIndent*/
    Baz;/*intersectionTypeNoIndent*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "formatBarOperator");
    f.verify_current_line_content(t, "type Union = number | {}");
    f.go_to_marker(t, "indent");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "autoformat");
    f.verify_current_line_content(t, "    | string");
    f.go_to_marker(t, "formatAmpersandOperator");
    f.verify_current_line_content(t, "type Intersection = Foo & Bar;");
    f.go_to_marker(t, "unionTypeNoIndent");
    f.verify_current_line_content(t, "    Bar |");
    f.go_to_marker(t, "intersectionTypeNoIndent");
    f.verify_current_line_content(t, "    Baz;");
    done();
}
