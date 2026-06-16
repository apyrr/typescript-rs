#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_string_literal_completions_for_generic_conditional_types_using_template_literal_types()
{
    let mut t = TestingT;
    run_test_string_literal_completions_for_generic_conditional_types_using_template_literal_types(
        &mut t,
    );
}

fn run_test_string_literal_completions_for_generic_conditional_types_using_template_literal_types(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r#"type PathOf<T, K extends string, P extends string = ""> =
  K extends ` + "`" + `${infer U}.${infer V}` + "`" + `
    ? U extends keyof T ? PathOf<T[U], V, ` + "`" + `${P}${U}.` + "`" + `> : ` + "`" + `${P}${keyof T & (string | number)}` + "`" + `
    : K extends keyof T ? ` + "`" + `${P}${K}` + "`" + ` : ` + "`" + `${P}${keyof T & (string | number)}` + "`" + `;

declare function consumer<K extends string>(path: PathOf<{a: string, b: {c: string}}, K>) : number;

consumer('b./*ts*/')"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["ts".to_string()]),
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
                    CompletionsExpectedItem::Label("a".to_string()),
                    CompletionsExpectedItem::Label("b".to_string()),
                    CompletionsExpectedItem::Label("b.c".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
