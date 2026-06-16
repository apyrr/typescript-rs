#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_js_doc_no_crash2() {
    let mut t = TestingT;
    run_test_completions_js_doc_no_crash2(&mut t);
}

fn run_test_completions_js_doc_no_crash2(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsJSDocNoCrash2") {
        return;
    }
    let content = r#"// @lib: es5
// @strict: true
// @filename: index.ts
/**
 * @example
  <file name="glyphicons.css">
    @import url(//netdna.bootstrapcdn.com/bootstrap/3.0.0/css/bootstrap-glyphicons.css);
  </file>
  <example module="ngAnimate" deps="angular-animate.js" animations="true">
    <file name="animations.css">
      .animate-show.ng-hide-add.ng-hide-add-active,
      .animate-show.ng-hide-remove.ng-hide-remove-active {
        transition:all linear 0./**/5s;
      }
    </file>
  </example>
 */
var ngShowDirective = ['$animate', function($animate) {}];"#;
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
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_global_types(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
