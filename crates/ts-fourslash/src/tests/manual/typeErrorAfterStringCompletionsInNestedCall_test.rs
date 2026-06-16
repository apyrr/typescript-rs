use crate::{
    is_suggestion_diagnostic, new_fourslash, CompletionsExpectedItem,
    CompletionsExpectedItemDefaults, CompletionsExpectedItems, CompletionsExpectedList,
    ExpectedCompletionEditRange, MarkerInput, TestingT,
};

pub fn test_type_error_after_string_completions_in_nested_call(t: &mut TestingT) {
    let content = r#"// @stableTypeOrdering: true
// @strict: true

type GreetingEvent =
  | { type: "MORNING" }
  | { type: "LUNCH_TIME" }
  | { type: "ALOHA" };

interface RaiseActionObject<TEvent extends { type: string }> {
  type: "raise";
  event: TEvent;
}

declare function raise<TEvent extends { type: string }>(
  ev: TEvent
): RaiseActionObject<TEvent>;

declare function createMachine<TEvent extends { type: string }>(config: {
  actions: RaiseActionObject<TEvent>;
}): void;

createMachine<GreetingEvent>({
  [|/*error*/actions|]: raise({ type: "ALOHA/*1*/" }),
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
                exact: ["ALOHA", "ALOHAx", "LUNCH_TIME", "MORNING"]
                    .into_iter()
                    .map(|label| CompletionsExpectedItem::Label(label.to_string()))
                    .collect(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    let diagnostics = f
        .get_diagnostics()
        .into_iter()
        .filter(|diagnostic| !is_suggestion_diagnostic(diagnostic))
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, 2322);
    assert_eq!(
        diagnostics[0].message.as_str(),
        "Type 'RaiseActionObject<{ type: \"ALOHAx\"; }>' is not assignable to type 'RaiseActionObject<GreetingEvent>'.\n  Type '{ type: \"ALOHAx\"; }' is not assignable to type 'GreetingEvent'.\n    Type '{ type: \"ALOHAx\"; }' is not assignable to type '{ type: \"ALOHA\"; }'.\n      Types of property 'type' are incompatible.\n        Type '\"ALOHAx\"' is not assignable to type '\"ALOHA\"'."
    );
    done();
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

