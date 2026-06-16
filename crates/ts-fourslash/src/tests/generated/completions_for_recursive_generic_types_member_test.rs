#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_for_recursive_generic_types_member() {
    let mut t = TestingT;
    run_test_completions_for_recursive_generic_types_member(&mut t);
}

fn run_test_completions_for_recursive_generic_types_member(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"export class TestBase<T extends TestBase<T>>
{
    public publicMethod(p: any): void {}
    private privateMethod(p: any): void {}
    protected protectedMethod(p: any): void {}
    public test(t: T): void
    {
        t./**/
    }
}";
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
                exact: vec![
                    CompletionsExpectedItem::Label("privateMethod".to_string()),
                    CompletionsExpectedItem::Label("protectedMethod".to_string()),
                    CompletionsExpectedItem::Label("publicMethod".to_string()),
                    CompletionsExpectedItem::Label("test".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
