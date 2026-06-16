#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_function_property() {
    let mut t = TestingT;
    run_test_function_property(&mut t);
}

fn run_test_function_property(t: &mut TestingT) {
    if should_skip_if_failing("TestFunctionProperty") {
        return;
    }
    let content = r"var a = {
    x(a: number) { }
};

var b = {
    x: function (a: number) { }
};

var c = {
    x: (a: number) => { }
};
a.x(/*signatureA*/1);
b.x(/*signatureB*/1);
c.x(/*signatureC*/1);
a./*completionA*/;
b./*completionB*/;
c./*completionC*/;
a./*quickInfoA*/x;
b./*quickInfoB*/x;
c./*quickInfoC*/x;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "signatureA");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("x(a: number): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "signatureB");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("x(a: number): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "signatureC");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("x(a: number): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.verify_completions(
        t,
        MarkerInput::Name("completionA".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "x".to_string(),
                    detail: Some("(method) x(a: number): void".to_string()),
                    ..Default::default()
                })],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["completionB".to_string(), "completionC".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "x".to_string(),
                    detail: Some("(property) x: (a: number) => void".to_string()),
                    ..Default::default()
                })],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_quick_info_at(t, "quickInfoA", "(method) x(a: number): void", "");
    f.verify_quick_info_at(t, "quickInfoB", "(property) x: (a: number) => void", "");
    f.verify_quick_info_at(t, "quickInfoC", "(property) x: (a: number) => void", "");
    done();
}
