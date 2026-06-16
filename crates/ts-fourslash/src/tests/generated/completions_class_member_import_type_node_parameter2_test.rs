#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_class_member_import_type_node_parameter2() {
    let mut t = TestingT;
    run_test_completions_class_member_import_type_node_parameter2(&mut t);
}

fn run_test_completions_class_member_import_type_node_parameter2(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsClassMemberImportTypeNodeParameter2") {
        return;
    }
    let content = r#"// @module: node18
// @FileName: /index.d.ts
export declare class Cls {
  method(
    param: import("./doesntexist.js").Foo,
  ): import("./doesntexist.js").Foo;
}

export declare class Derived extends Cls {
  /*1*/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("1".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "method".to_string(),
                    insert_text: Some(
                        "method(param: import(\"./doesntexist.js\").Foo);".to_string(),
                    ),
                    filter_text: Some("method".to_string()),
                    ..Default::default()
                })],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
