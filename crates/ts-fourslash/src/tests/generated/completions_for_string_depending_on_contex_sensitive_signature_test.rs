#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_for_string_depending_on_contex_sensitive_signature() {
    let mut t = TestingT;
    run_test_completions_for_string_depending_on_contex_sensitive_signature(&mut t);
}

fn run_test_completions_for_string_depending_on_contex_sensitive_signature(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsForStringDependingOnContexSensitiveSignature") {
        return;
    }
    let content = r#"// @strict: true

type ActorRef<TEvent extends { type: string }> = {
  send: (ev: TEvent) => void
}

type Action<TContext> = {
  (ctx: TContext): void
}

type Config<TContext> = {
  entry: Action<TContext>
}

declare function createMachine<TContext>(config: Config<TContext>): void

type EventFrom<T> = T extends ActorRef<infer TEvent> ? TEvent : never

declare function sendTo<
  TContext,
  TActor extends ActorRef<any>
>(
  actor: ((ctx: TContext) => TActor),
  event: EventFrom<TActor>
): Action<TContext>

createMachine<{
  child: ActorRef<{ type: "EVENT" }>;
}>({
  entry: sendTo((ctx) => ctx.child, { type: /*1*/ }),
});

createMachine<{
  child: ActorRef<{ type: "EVENT" }>;
}>({
  entry: sendTo((ctx) => ctx.child, { type: "/*2*/" }),
});"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("1".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Label("\"EVENT\"".to_string())],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("2".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("EVENT".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
