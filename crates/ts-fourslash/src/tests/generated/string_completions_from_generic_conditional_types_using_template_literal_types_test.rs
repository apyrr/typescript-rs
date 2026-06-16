#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_string_completions_from_generic_conditional_types_using_template_literal_types() {
    let mut t = TestingT;
    run_test_string_completions_from_generic_conditional_types_using_template_literal_types(&mut t);
}

fn run_test_string_completions_from_generic_conditional_types_using_template_literal_types(
    t: &mut TestingT,
) {
    if should_skip_if_failing(
        "TestStringCompletionsFromGenericConditionalTypesUsingTemplateLiteralTypes",
    ) {
        return;
    }
    let content = r#"// @stableTypeOrdering: true
// @strict: true
type keyword = "foo" | "bar" | "baz"

type validateString<s> = s extends keyword
    ? s
    : s extends `${infer left extends keyword}|${infer right}`
    ? right extends keyword
        ? s
        : `${left}|${keyword}`
    : keyword

type isUnknown<t> = unknown extends t
    ? [t] extends [{}]
        ? false
        : true
    : false

type validate<def> = def extends string
    ? validateString<def>
    : isUnknown<def> extends true
    ? keyword
    : {
          [k in keyof def]: validate<def[k]>
      }
const parse = <def>(def: validate<def>) => def
const shallowExpression = parse("foo|/*ts*/")
const nestedExpression = parse({ prop: "foo|/*ts2*/" })"#;
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
                    CompletionsExpectedItem::Label("bar".to_string()),
                    CompletionsExpectedItem::Label("baz".to_string()),
                    CompletionsExpectedItem::Label("foo".to_string()),
                    CompletionsExpectedItem::Label("foo|bar".to_string()),
                    CompletionsExpectedItem::Label("foo|baz".to_string()),
                    CompletionsExpectedItem::Label("foo|foo".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["ts2".to_string()]),
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
                    CompletionsExpectedItem::Label("foo|bar".to_string()),
                    CompletionsExpectedItem::Label("foo|baz".to_string()),
                    CompletionsExpectedItem::Label("foo|foo".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
