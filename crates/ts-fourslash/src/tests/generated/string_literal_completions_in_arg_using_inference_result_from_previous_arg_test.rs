#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_string_literal_completions_in_arg_using_inference_result_from_previous_arg() {
    let mut t = TestingT;
    run_test_string_literal_completions_in_arg_using_inference_result_from_previous_arg(&mut t);
}

fn run_test_string_literal_completions_in_arg_using_inference_result_from_previous_arg(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r#"// @strict: true
// https://github.com/microsoft/TypeScript/issues/55545
enum myEnum {
  valA = "valA",
  valB = "valB",
}

interface myEnumParamMapping {
  ["valA"]: "1" | "2";
  ["valB"]: "3" | "4";
}

function myFunction<K extends keyof typeof myEnum>(
  a: K,
  b: myEnumParamMapping[K],
) {}

myFunction("valA", "/*ts1*/");
myFunction("valA", ` + "`" + `/*ts2*/` + "`" + `);

function myFunction2<K extends keyof typeof myEnum>(
  a: K,
  { b }: { b: myEnumParamMapping[K] },
) {}

myFunction2("valA", { b: "/*ts3*/" });
myFunction2("valA", { b: ` + "`" + `/*ts4*/` + "`" + ` });"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "ts1".to_string(),
            "ts2".to_string(),
            "ts3".to_string(),
            "ts4".to_string(),
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
                exact: vec![
                    CompletionsExpectedItem::Label("1".to_string()),
                    CompletionsExpectedItem::Label("2".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
