#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_class_member_import_type_node_parameter1() {
    let mut t = TestingT;
    run_test_completions_class_member_import_type_node_parameter1(&mut t);
}

fn run_test_completions_class_member_import_type_node_parameter1(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsClassMemberImportTypeNodeParameter1") {
        return;
    }
    let content = r#"// @module: node18
// @Filename: /generation.d.ts
export type GenerationConfigType = { max_length?: number };
// @FileName: /index.d.ts
export declare class PreTrainedModel {
  _get_generation_config(
    param: import("./generation.js").GenerationConfigType,
  ): import("./generation.js").GenerationConfigType;
}

export declare class BlenderbotSmallPreTrainedModel extends PreTrainedModel {
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
        label: "_get_generation_config".to_string(),
        insert_text: Some("_get_generation_config(param: import(\"./generation.js\").GenerationConfigType): import(\"./generation.js\").GenerationConfigType;".to_string()),
        filter_text: Some("_get_generation_config".to_string()),
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
