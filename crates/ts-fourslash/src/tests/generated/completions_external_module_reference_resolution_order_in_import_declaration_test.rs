#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_external_module_reference_resolution_order_in_import_declaration() {
    let mut t = TestingT;
    run_test_completions_external_module_reference_resolution_order_in_import_declaration(&mut t);
}

fn run_test_completions_external_module_reference_resolution_order_in_import_declaration(
    t: &mut TestingT,
) {
    if should_skip_if_failing(
        "TestCompletionsExternalModuleReferenceResolutionOrderInImportDeclaration",
    ) {
        return;
    }
    let content = r#"// @Filename: externalModuleRefernceResolutionOrderInImportDeclaration_file1.ts
export function foo() { };
// @Filename: externalModuleRefernceResolutionOrderInImportDeclaration_file2.ts
declare module "externalModuleRefernceResolutionOrderInImportDeclaration_file1" {
    export function bar();
}
// @Filename: externalModuleRefernceResolutionOrderInImportDeclaration_file3.ts
///<reference path='externalModuleRefernceResolutionOrderInImportDeclaration_file2.ts'/>
import file1 = require('externalModuleRefernceResolutionOrderInImportDeclaration_file1');
/*1*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "file1.");
    f.verify_completions(
        t,
        MarkerInput::None,
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Label("bar".to_string())],
                excludes: vec!["foo".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
