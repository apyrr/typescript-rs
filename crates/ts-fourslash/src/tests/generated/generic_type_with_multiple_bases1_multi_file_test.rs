#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_type_with_multiple_bases1_multi_file() {
    let mut t = TestingT;
    run_test_generic_type_with_multiple_bases1_multi_file(&mut t);
}

fn run_test_generic_type_with_multiple_bases1_multi_file(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: genericTypeWithMultipleBases_0.ts
interface iBaseScope {
    watch: () => void;
}
// @Filename: genericTypeWithMultipleBases_1.ts
interface iMover {
    moveUp: () => void;
}
// @Filename: genericTypeWithMultipleBases_2.ts
interface iScope<TModel> extends iBaseScope, iMover {
    family: TModel;
}
// @Filename: genericTypeWithMultipleBases_3.ts
var x: iScope<number>;
// @Filename: genericTypeWithMultipleBases_4.ts
x./**/";
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
                includes: vec![
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "watch".to_string(),
                        detail: Some("(property) iBaseScope.watch: () => void".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "moveUp".to_string(),
                        detail: Some("(property) iMover.moveUp: () => void".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "family".to_string(),
                        detail: Some("(property) iScope<number>.family: number".to_string()),
                        ..Default::default()
                    }),
                ],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
