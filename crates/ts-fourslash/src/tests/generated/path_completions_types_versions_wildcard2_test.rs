#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_path_completions_types_versions_wildcard2() {
    let mut t = TestingT;
    run_test_path_completions_types_versions_wildcard2(&mut t);
}

fn run_test_path_completions_types_versions_wildcard2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @module: commonjs
// @resolveJsonModule: false
// @Filename: /node_modules/foo/package.json
{
  "types": "index.d.ts",
  "typesVersions": {
    "<=3.4.1": {
      "*": ["ts-old/*"]
    }
  }
}
// @Filename: /node_modules/foo/nope.d.ts
export const nope = 0;
// @Filename: /node_modules/foo/ts-old/index.d.ts
export const index = 0;
// @Filename: /node_modules/foo/ts-old/blah.d.ts
export const blah = 0;
// @Filename: /node_modules/foo/ts-old/subfolder/one.d.ts
export const one = 0;
// @Filename: /a.ts
import { } from "foo//**/";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![
                    CompletionsExpectedItem::Label("nope".to_string()),
                    CompletionsExpectedItem::Label("ts-old".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
