#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_clone_question_token() {
    let mut t = TestingT;
    run_test_completion_clone_question_token(&mut t);
}

fn run_test_completion_clone_question_token(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
// @Filename: /file2.ts
type TCallback<T = any> = (options: T) => any;
type InKeyOf<E> = { [K in keyof E]?: TCallback<E[K]>; };
export class Bar<A> {
    baz(a: InKeyOf<A>): void { }
}
// @Filename: /file1.ts
import { Bar } from './file2';
type TwoKeys = Record<'a' | 'b', { thisFails?: any; }>
class Foo extends Bar<TwoKeys> {
    /**/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("".to_string()), Some(&CompletionsExpectedList {
    is_incomplete: false,
    item_defaults: Some(CompletionsExpectedItemDefaults {
        commit_characters: Some(Vec::new()),
        edit_range: ExpectedCompletionEditRange::Ignored,
    }),
    items: Some(CompletionsExpectedItems {
        includes: vec![
CompletionsExpectedItem::Item(lsproto::CompletionItem {
        label: "baz".to_string(),
        insert_text: Some("baz(a: { a?: (options: { thisFails?: any; }) => any; b?: (options: { thisFails?: any; }) => any; })): void {\n}".to_string()),
        filter_text: Some("baz".to_string()),
        ..Default::default()
    }),
],
        excludes: Vec::new(),
        exact: Vec::new(),
        unsorted: Vec::new(),
    }),
    user_preferences: None,
}));
    done();
}
