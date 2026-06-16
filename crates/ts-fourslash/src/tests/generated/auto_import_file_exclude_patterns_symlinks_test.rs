#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_file_exclude_patterns_symlinks() {
    let mut t = TestingT;
    run_test_auto_import_file_exclude_patterns_symlinks(&mut t);
}

fn run_test_auto_import_file_exclude_patterns_symlinks(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportFileExcludePatterns_symlinks") {
        return;
    }
    let content = r#"// @lib: es5
// @module: commonjs
// @Filename: /home/src/workspaces/project/node_modules/.store/@remix-run-server-runtime-virtual-c72daf0d/package/package.json
{
  "name": "@remix-run/server-runtime",
  "version": "0.0.0",
  "main": "index.js"
}
// @Filename: /home/src/workspaces/project/node_modules/.store/@remix-run-server-runtime-virtual-c72daf0d/package/index.d.ts
export declare function ServerRuntimeMetaFunction(): void;
// @Filename: /home/src/workspaces/project/package.json
{ "dependencies": { "@remix-run/server-runtime": "*" } }
// @link: /home/src/workspaces/project/node_modules/.store/@remix-run-server-runtime-virtual-c72daf0d/package -> /home/src/workspaces/project/node_modules/@remix-run/server-runtime
// @Filename: /home/src/workspaces/project/index.ts
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
                    "/**/@remix-run/server-runtime".to_string(),
                ],
                ..Default::default()
            }),
        }),
    );
    f.verify_import_fix_at_position(
        t,
        &[],
        Some(UserPreferences {
            auto_import_file_exclude_patterns: vec!["/**/@remix-run/server-runtime".to_string()],
            ..Default::default()
        }),
    );
    done();
}
