#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_augmented_types_module3() {
    let mut t = TestingT;
    run_test_augmented_types_module3(&mut t);
}

fn run_test_augmented_types_module3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function m2g() { };
namespace m2g { export class C { foo(x: number) { } } }
var x: m2g./*1*/;
var /*2*/r = m2g/*3*/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("1".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("C".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.insert(t, "C.");
    f.verify_completions(t, MarkerInput::None, None);
    f.backspace(t, 1);
    f.verify_quick_info_at(t, "2", "var r: typeof m2g", "");
    f.go_to_marker(t, "3");
    f.insert(t, "(");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("m2g(): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
