#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_writing_spread_argument() {
    let mut t = TestingT;
    run_test_completions_writing_spread_argument(&mut t);
}

fn run_test_completions_writing_spread_argument(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsWritingSpreadArgument") {
        return;
    }
    let content = r"// @lib: es5

const [] = [Math.min(./*marker*/)]
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "marker");
    f.verify_completions(t, MarkerInput::None, None);
    f.insert(t, ".");
    f.verify_completions(t, MarkerInput::None, None);
    f.insert(t, ".");
    f.verify_completions(
        t,
        MarkerInput::None,
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_globals(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
