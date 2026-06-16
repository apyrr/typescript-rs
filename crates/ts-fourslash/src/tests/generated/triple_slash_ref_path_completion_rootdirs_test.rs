#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_triple_slash_ref_path_completion_rootdirs() {
    let mut t = TestingT;
    run_test_triple_slash_ref_path_completion_rootdirs(&mut t);
}

fn run_test_triple_slash_ref_path_completion_rootdirs(t: &mut TestingT) {
    if should_skip_if_failing("TestTripleSlashRefPathCompletionRootdirs") {
        return;
    }
    let content = r#"// @rootDirs: sub/src1,src2
// @Filename: src2/test0.ts
/// <reference path="./mo/*0*/
// @Filename: src2/module0.ts
export var w = 0;
// @Filename: sub/src1/module1.ts
export var x = 0;
// @Filename: sub/src1/module2.ts
export var y = 0;
// @Filename: sub/src1/more/module3.ts
export var z = 0;
// @Filename: f1.ts
/*f1*/
// @Filename: f2.tsx
/*f2*/
// @Filename: folder/f1.ts
/*subf1*/
// @Filename: f3.js
/*f3*/
// @Filename: f4.jsx
/*f4*/
// @Filename: e1.ts
/*e1*/
// @Filename: e2.js
/*e2*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("0".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("module0.ts".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
