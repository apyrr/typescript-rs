#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_of_interface_and_var() {
    let mut t = TestingT;
    run_test_completion_of_interface_and_var(&mut t);
}

fn run_test_completion_of_interface_and_var(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
interface AnalyserNode {
}
declare var AnalyserNode: {
    prototype: AnalyserNode;
    new(): AnalyserNode;
};
/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("".to_string()), Some(&CompletionsExpectedList {
    is_incomplete: false,
    item_defaults: Some(CompletionsExpectedItemDefaults {
        commit_characters: Some(default_commit_characters()),
        edit_range: ExpectedCompletionEditRange::Ignored,
    }),
    items: Some(CompletionsExpectedItems {
        includes: vec![
CompletionsExpectedItem::Item(lsproto::CompletionItem {
        label: "AnalyserNode".to_string(),
        detail: Some("interface AnalyserNode\nvar analyser_node: {\n    new (): AnalyserNode;\n    prototype: AnalyserNode;\n})".to_string()),
        kind: Some(lsproto::CompletionItemKind::VARIABLE),
        ..Default::default()
    }),
],
        excludes: Vec::new(),
        exact: Vec::new(),
        unsorted: Vec::new(),
    }),
    user_preferences: None,
}));
    done();
}
