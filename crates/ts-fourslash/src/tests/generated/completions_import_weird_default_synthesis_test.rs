#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_weird_default_synthesis() {
    let mut t = TestingT;
    run_test_completions_import_weird_default_synthesis(&mut t);
}

fn run_test_completions_import_weird_default_synthesis(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsImport_weirdDefaultSynthesis") {
        return;
    }
    let content = r"// @module: commonjs
// @esModuleInterop: false
// @allowSyntheticDefaultImports: false
// @Filename: /collection.ts
class Collection {
  public static readonly default: typeof Collection = Collection;
}
export = Collection as typeof Collection & { default: typeof Collection };
// @Filename: /index.ts
Colle/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_apply_code_action_from_completion(
        t,
        Some(""),
        &ApplyCodeActionFromCompletionOptions {
            name: "Collection".to_string(),
            source: "./collection".to_string(),
            auto_import_fix: None,
            description: "Add import from \"./collection\"".to_string(),
            new_file_content: Some(
                r#"import Collection = require("./collection");

Colle"#
                    .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    done();
}
