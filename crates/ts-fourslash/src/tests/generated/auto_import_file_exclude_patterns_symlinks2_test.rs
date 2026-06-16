#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_file_exclude_patterns_symlinks2() {
    let mut t = TestingT;
    run_test_auto_import_file_exclude_patterns_symlinks2(&mut t);
}

fn run_test_auto_import_file_exclude_patterns_symlinks2(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportFileExcludePatterns_symlinks2") {
        return;
    }
    let content = r#"// @lib: es5
// @module: commonjs
// @Filename: c:/workspaces/project/node_modules/.store/aws-sdk-virtual-adfe098/package/package.json
{ "name": "aws-sdk", "version": "2.0.0", "main": "index.js" }
// @Filename: c:/workspaces/project/node_modules/.store/aws-sdk-virtual-adfe098/package/index.d.ts
export {};
// @Filename: c:/workspaces/project/node_modules/@remix-run/server-runtime/package.json
{
  "name": "@remix-run/server-runtime",
  "version": "0.0.0",
  "main": "index.js"
}
// @Filename: c:/workspaces/project/node_modules/@remix-run/server-runtime/index.d.ts
export declare function ServerRuntimeMetaFunction(): void;
// @Filename: c:/workspaces/project/package.json
{ "dependencies": { "aws-sdk": "*", "@remix-run/server-runtime": "*" } }
// @link: c:/workspaces/project/node_modules/.store/aws-sdk-virtual-adfe098/package -> c:/workspaces/project/node_modules/aws-sdk
// @Filename: c:/workspaces/project/index.ts
ServerRuntimeMetaFunction/**/"#;
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
                excludes: vec!["ServerRuntimeMetaFunction".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: Some(UserPreferences {
                auto_import_file_exclude_patterns: vec![
                    "c:/**/@remix-run/server-runtime".to_string(),
                ],
                ..Default::default()
            }),
        }),
    );
    f.verify_import_fix_at_position(
        t,
        &[],
        Some(UserPreferences {
            auto_import_file_exclude_patterns: vec!["c:/**/@remix-run/server-runtime".to_string()],
            ..Default::default()
        }),
    );
    done();
}
