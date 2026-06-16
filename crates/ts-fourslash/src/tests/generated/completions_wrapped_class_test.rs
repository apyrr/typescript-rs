#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_wrapped_class() {
    let mut t = TestingT;
    run_test_completions_wrapped_class(&mut t);
}

fn run_test_completions_wrapped_class(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsWrappedClass") {
        return;
    }
    let content = r"class Client {
    private close() { }
    public open() { }
}
type Wrap<T> = T &
{
    [K in Extract<keyof T, string> as `${K}Wrapped`]: T[K];
};
class Service {
    method() {
        let service = undefined as unknown as Wrap<Client>;
        const { /*a*/ } = service;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("a".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![
                    CompletionsExpectedItem::Label("open".to_string()),
                    CompletionsExpectedItem::Label("openWrapped".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
