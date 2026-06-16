#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_literal_overload() {
    let mut t = TestingT;
    run_test_completions_literal_overload(&mut t);
}

fn run_test_completions_literal_overload(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowJs: true
// @Filename: /a.tsx
interface Events {
  "": any;
  drag: any;
  dragenter: any;
}
declare function addListener<K extends keyof Events>(type: K, listener: (ev: Events[K]) => any): void;

declare function ListenerComponent<K extends keyof Events>(props: { type: K, onWhatever: (ev: Events[K]) => void }): JSX.Element;

addListener("/*ts*/");
(<ListenerComponent type="/*tsx*/" />);
// @Filename: /b.js
addListener("/*js*/");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["ts".to_string(), "tsx".to_string(), "js".to_string()]),
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
                    CompletionsExpectedItem::Label("".to_string()),
                    CompletionsExpectedItem::Label("drag".to_string()),
                    CompletionsExpectedItem::Label("dragenter".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.insert(t, "drag");
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["ts".to_string(), "tsx".to_string(), "js".to_string()]),
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
                    CompletionsExpectedItem::Label("".to_string()),
                    CompletionsExpectedItem::Label("drag".to_string()),
                    CompletionsExpectedItem::Label("dragenter".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
