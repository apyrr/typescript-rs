#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_completions_generic_component() {
    let mut t = TestingT;
    run_test_tsx_completions_generic_component(&mut t);
}

fn run_test_tsx_completions_generic_component(t: &mut TestingT) {
    if should_skip_if_failing("TestTsxCompletionsGenericComponent") {
        return;
    }
    let content = r"// @jsx: preserve
// @skipLibCheck: true
// @Filename: file.tsx
 declare namespace JSX {
     interface Element { }
     interface IntrinsicElements {
     }
     interface ElementAttributesProperty { props; }
 }

class Table<P> {
    constructor(public props: P) {}
}

type Props = { widthInCol: number; text: string; };

/**
 * @param width {number} Table width in px
 */
function createTable(width) {
    return <Table<Props> /*1*/ />
}

createTable(800);";
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
                includes: vec![
                    CompletionsExpectedItem::Label("widthInCol".to_string()),
                    CompletionsExpectedItem::Label("text".to_string()),
                ],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
