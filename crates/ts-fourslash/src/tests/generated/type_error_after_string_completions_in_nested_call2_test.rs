#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_type_error_after_string_completions_in_nested_call2() {
    let mut t = TestingT;
    run_test_type_error_after_string_completions_in_nested_call2(&mut t);
}

fn run_test_type_error_after_string_completions_in_nested_call2(t: &mut TestingT) {
    if should_skip_if_failing("TestTypeErrorAfterStringCompletionsInNestedCall2") {
        return;
    }
    let content = r#"// @stableTypeOrdering: true
// @strict: true

type ActionFunction<
  TExpressionEvent extends { type: string },
  out TEvent extends { type: string }
> = {
  ({ event }: { event: TExpressionEvent }): void;
  _out_TEvent?: TEvent;
};

interface MachineConfig<TEvent extends { type: string }> {
  types: {
    events: TEvent;
  };
  on: {
    [K in TEvent["type"]]?: ActionFunction<
      Extract<TEvent, { type: K }>,
      TEvent
    >;
  };
}

declare function raise<
  TExpressionEvent extends { type: string },
  TEvent extends { type: string }
>(
  resolve: ({ event }: { event: TExpressionEvent }) => TEvent
): {
  ({ event }: { event: TExpressionEvent }): void;
  _out_TEvent?: TEvent;
};

declare function createMachine<TEvent extends { type: string }>(
  config: MachineConfig<TEvent>
): void;

createMachine({
  types: {
    events: {} as { type: "FOO" } | { type: "BAR" },
  },
  on: {
    [|/*error*/FOO|]: raise(({ event }) => {
      return {
        type: "BAR/*1*/" as const,
      };
    }),
  },
});"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "x");
    f.verify_completions(
        t,
        MarkerInput::None,
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
                    CompletionsExpectedItem::Label("BAR".to_string()),
                    CompletionsExpectedItem::Label("FOO".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_baseline_non_suggestion_diagnostics(t);
    done();
}
