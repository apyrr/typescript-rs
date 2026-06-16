#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_generic_indexed_access6() {
    let mut t = TestingT;
    run_test_completions_generic_indexed_access6(&mut t);
}

fn run_test_completions_generic_indexed_access6(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsGenericIndexedAccess6") {
        return;
    }
    let content = r#"// @Filename: component.tsx
interface CustomElements {
  'component-one': {
      foo?: string;
  },
  'component-two': {
      bar?: string;
  }
}

type Options<T extends keyof CustomElements> = { kind: T } & Required<{ x: CustomElements[(T extends string ? T : never) & string] }['x']>;

declare function Component<T extends keyof CustomElements>(props: Options<T>): void;

const c = <Component /**/ kind="component-one" />"#;
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
                exact: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "foo".to_string(),
                    ..Default::default()
                })],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
