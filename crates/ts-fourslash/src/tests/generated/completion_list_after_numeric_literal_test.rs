#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_after_numeric_literal() {
    let mut t = TestingT;
    run_test_completion_list_after_numeric_literal(&mut t);
}

fn run_test_completion_list_after_numeric_literal(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: f1.ts
0./*dotOnNumberExpressions1*/
// @Filename: f2.ts
0.0./*dotOnNumberExpressions2*/
// @Filename: f3.ts
0.0.0./*dotOnNumberExpressions3*/
// @Filename: f4.ts
0./** comment *//*dotOnNumberExpressions4*/
// @Filename: f5.ts
(0)./*validDotOnNumberExpressions1*/
// @Filename: f6.ts
(0.)./*validDotOnNumberExpressions2*/
// @Filename: f7.ts
(0.0)./*validDotOnNumberExpressions3*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "dotOnNumberExpressions1".to_string(),
            "dotOnNumberExpressions4".to_string(),
        ]),
        None,
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "dotOnNumberExpressions2".to_string(),
            "dotOnNumberExpressions3".to_string(),
            "validDotOnNumberExpressions1".to_string(),
            "validDotOnNumberExpressions2".to_string(),
            "validDotOnNumberExpressions3".to_string(),
        ]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Label("toExponential".to_string())],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
