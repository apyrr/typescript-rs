#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_literal_from_inference_within_inferred_type2() {
    let mut t = TestingT;
    run_test_completions_literal_from_inference_within_inferred_type2(&mut t);
}

fn run_test_completions_literal_from_inference_within_inferred_type2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /a.tsx
type Values<T> = T[keyof T];

type GetStates<T> = T extends { states: object } ? T["states"] : never;

type IsNever<T> = [T] extends [never] ? 1 : 0;

type GetIds<T, Gathered extends string = never> = IsNever<T> extends 1
  ? Gathered
  : "id" extends keyof T
  ? GetIds<Values<GetStates<T>>, Gathered | ` + "`" + `#${T["id"] & string}` + "`" + `>
  : GetIds<Values<GetStates<T>>, Gathered>;

type StateConfig<
  TStates extends Record<string, StateConfig> = Record<
    string,
    StateConfig<any>
  >,
  TIds extends string = string
> = {
  id?: string;
  initial?: keyof TStates & string;
  states?: {
    [K in keyof TStates]: StateConfig<GetStates<TStates[K]>, TIds>;
  };
  on?: Record<string, TIds | ` + "`" + `.${keyof TStates & string}` + "`" + `>;
};

declare function createMachine<const T extends StateConfig<GetStates<T>, GetIds<T>>>(
  config: T
): void;

createMachine({
  initial: "child",
  states: {
    child: {
      initial: "foo",
      states: {
        foo: {
          id: "wow_deep_id",
        },
      },
    },
  },
  on: {
    EV: "/*ts*/",
  },
});"#;
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
                    CompletionsExpectedItem::Label("#wow_deep_id".to_string()),
                    CompletionsExpectedItem::Label(".child".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
