#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_file_exclude_patterns2() {
    let mut t = TestingT;
    run_test_auto_import_file_exclude_patterns2(&mut t);
}

fn run_test_auto_import_file_exclude_patterns2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @lib: es5
// @module: commonjs
// @Filename: /home/src/workspaces/project/node_modules/aws-sdk/package.json
{ "name": "aws-sdk", "version": "2.0.0", "main": "index.js" }
// @Filename: /home/src/workspaces/project/node_modules/aws-sdk/index.d.ts
export * from "./clients/s3";
// @Filename: /home/src/workspaces/project/node_modules/aws-sdk/clients/s3.d.ts
export declare class S3 {}
// @Filename: /home/src/workspaces/project/package.json
{ "dependencies": { "aws-sdk": "*" } }
// @Filename: /home/src/workspaces/project/index.ts
S3/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
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
                excludes: vec!["S3".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: Some(UserPreferences {
                auto_import_file_exclude_patterns: vec!["**/node_modules/aws-sdk".to_string()],
                ..Default::default()
            }),
        }),
    );
    f.verify_import_fix_at_position(
        t,
        &[],
        Some(UserPreferences {
            auto_import_file_exclude_patterns: vec!["**/node_modules/aws-sdk".to_string()],
            ..Default::default()
        }),
    );
    done();
}
