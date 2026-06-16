#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_at_invalid_locations() {
    let mut t = TestingT;
    run_test_completion_list_at_invalid_locations(&mut t);
}

fn run_test_completion_list_at_invalid_locations(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"var v1 = '';
" /*openString1*/
var v2 = '';
"/*openString2*/
var v3 = '';
" bar./*openString3*/
var v4 = '';
// bar./*inComment1*/
var v6 = '';
// /*inComment2*/
var v7 = '';
/* /*inComment3*/
var v11 = '';
  // /*inComment4*/
var v12 = '';
type htm/*inTypeAlias*/

//  /*inComment5*/
foo;
var v10 = /reg/*inRegExp1*/ex/;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "openString1".to_string(),
            "openString2".to_string(),
            "openString3".to_string(),
        ]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "inComment1".to_string(),
            "inComment2".to_string(),
            "inComment3".to_string(),
            "inComment4".to_string(),
            "inTypeAlias".to_string(),
            "inComment5".to_string(),
            "inRegExp1".to_string(),
        ]),
        None,
    );
    done();
}
