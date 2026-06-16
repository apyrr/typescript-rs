#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_meaning() {
    let mut t = TestingT;
    run_test_quick_info_meaning(&mut t);
}

fn run_test_quick_info_meaning(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoMeaning") {
        return;
    }
    let content = r#"// @lib: es5
// @module: commonjs
// @Filename: foo.d.ts
declare const [|/*foo_value_declaration*/foo: number|];
[|declare module "foo_module" {
    interface /*foo_type_declaration*/I { x: number; y: number }
    export = I;
}|]
// @Filename: foo_user.ts
///<reference path="foo.d.ts" />
[|import foo = require("foo_module");|]
const x = foo/*foo_value*/;
const i: foo/*foo_type*/ = { x: 1, y: 2 };
// @Filename: bar.d.ts
[|declare interface /*bar_type_declaration*/bar { x: number; y: number }|]
[|declare module "bar_module" {
    const /*bar_value_declaration*/x: number;
    export = x;
}|]
// @Filename: bar_user.ts
///<reference path="bar.d.ts" />
[|import bar = require("bar_module");|]
const x = bar/*bar_value*/;
const i: bar/*bar_type*/ = { x: 1, y: 2 };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_workspace_symbol(&[workspace_symbol_case(
        "foo",
        vec![
            symbol_information(
                "foo",
                lsproto::SymbolKindVariable,
                f.ranges()[0].ls_location(),
                None,
            ),
            symbol_information(
                "foo",
                lsproto::SymbolKindVariable,
                f.ranges()[2].ls_location(),
                None,
            ),
            symbol_information(
                "foo_module",
                lsproto::SymbolKindNamespace,
                f.ranges()[1].ls_location(),
                None,
            ),
        ],
    )]);
    f.go_to_marker(t, "foo_value");
    f.verify_quick_info_is(t, "const foo: number", "");
    f.go_to_marker(t, "foo_type");
    f.verify_quick_info_is(
        t,
        "(alias) interface foo\nimport foo = require(\"foo_module\")",
        "",
    );
    f.verify_workspace_symbol(&[workspace_symbol_case(
        "bar",
        vec![
            symbol_information(
                "bar",
                lsproto::SymbolKindInterface,
                f.ranges()[3].ls_location(),
                None,
            ),
            symbol_information(
                "bar",
                lsproto::SymbolKindVariable,
                f.ranges()[5].ls_location(),
                None,
            ),
            symbol_information(
                "bar_module",
                lsproto::SymbolKindNamespace,
                f.ranges()[4].ls_location(),
                None,
            ),
        ],
    )]);
    f.go_to_marker(t, "bar_value");
    f.verify_quick_info_is(
        t,
        "(alias) const bar: number\nimport bar = require(\"bar_module\")",
        "",
    );
    f.go_to_marker(t, "bar_type");
    f.verify_quick_info_is(t, "interface bar", "");
    f.verify_baseline_go_to_definition(
        t,
        &[
            "foo_value".to_string(),
            "foo_type".to_string(),
            "bar_value".to_string(),
            "bar_type".to_string(),
        ],
    );
    done();
}
