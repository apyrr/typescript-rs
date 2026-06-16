#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_class_member_import_type_node_parameter3() {
    let mut t = TestingT;
    run_test_completions_class_member_import_type_node_parameter3(&mut t);
}

fn run_test_completions_class_member_import_type_node_parameter3(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsClassMemberImportTypeNodeParameter3") {
        return;
    }
    let content = r#"// @module: node18
// @FileName: /other/foo.d.ts
export declare type Bar = { baz: string };
// @FileName: /other/cls.d.ts
export declare class Cls {
  method(
    param: import("./foo.js").Bar,
  ): import("./foo.js").Bar;
}
// @FileName: /index.d.ts
import { Cls } from "./other/cls.js";

export declare class Derived extends Cls {
  /*1*/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("1".to_string()), Some(&CompletionsExpectedList {
    is_incomplete: false,
    item_defaults: Some(CompletionsExpectedItemDefaults {
        commit_characters: Some(Vec::new()),
        edit_range: ExpectedCompletionEditRange::Ignored,
    }),
    items: Some(CompletionsExpectedItems {
        includes: vec![
CompletionsExpectedItem::Item(lsproto::CompletionItem {
        label: "method".to_string(),
        insert_text: Some("method(param: import(\"./other/foo.js\").Bar): import(\"./other/foo.js\").Bar;".to_string()),
        filter_text: Some("method".to_string()),
        ..Default::default()
    }),
],
        excludes: Vec::new(),
        exact: Vec::new(),
        unsorted: Vec::new(),
    }),
    user_preferences: None,
}));
    done();
}
