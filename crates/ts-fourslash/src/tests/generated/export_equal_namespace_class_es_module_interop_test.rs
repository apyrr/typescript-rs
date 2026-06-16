#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_export_equal_namespace_class_es_module_interop() {
    let mut t = TestingT;
    run_test_export_equal_namespace_class_es_module_interop(&mut t);
}

fn run_test_export_equal_namespace_class_es_module_interop(t: &mut TestingT) {
    if should_skip_if_failing("TestExportEqualNamespaceClassESModuleInterop") {
        return;
    }
    let content = r#"// @esModuleInterop: true
// @moduleResolution: bundler
// @target: es2015
// @module: esnext
// @Filename: /node_modules/@bar/foo/index.d.ts
export = Foo;
declare class Foo {}
declare namespace Foo {}  // class/namespace declaration causes the issue
// @Filename: /node_modules/foo/index.d.ts
import * as Foo from "@bar/foo";
export = Foo;
// @Filename: /index.ts
import Foo from "foo";
/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/index.ts");
    f.verify_completions(
        t,
        MarkerInput::Name("".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Label("Foo".to_string())],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
