#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_declaration_attributes_empty_module_specifier1() {
    let mut t = TestingT;
    run_test_completions_import_declaration_attributes_empty_module_specifier1(&mut t);
}

fn run_test_completions_import_declaration_attributes_empty_module_specifier1(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsImportDeclarationAttributesEmptyModuleSpecifier1") {
        return;
    }
    let content = r#"// @strict: true
// @filename: global.d.ts
interface ImportAttributes { 
  type: "json";
}
// @filename: index.ts
import * as ns from "" with { type: "/**/" };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
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
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("json".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
