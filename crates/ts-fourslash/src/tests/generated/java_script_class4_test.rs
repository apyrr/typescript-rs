#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_java_script_class4() {
    let mut t = TestingT;
    run_test_java_script_class4(&mut t);
}

fn run_test_java_script_class4(t: &mut TestingT) {
    if should_skip_if_failing("TestJavaScriptClass4") {
        return;
    }
    let content = r"// @allowNonTsExtensions: true
// @Filename: Foo.js
class Foo {
   constructor() {
       /**
         * @type {string}
       */
       this.baz = null;
   }
}
var x = new Foo();
x/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, ".baz.");
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
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "substring".to_string(),
                    kind: Some(lsproto::CompletionItemKind::METHOD),
                    ..Default::default()
                })],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
