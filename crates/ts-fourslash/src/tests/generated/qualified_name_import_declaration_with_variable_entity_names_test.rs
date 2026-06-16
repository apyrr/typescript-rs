#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_qualified_name_import_declaration_with_variable_entity_names() {
    let mut t = TestingT;
    run_test_qualified_name_import_declaration_with_variable_entity_names(&mut t);
}

fn run_test_qualified_name_import_declaration_with_variable_entity_names(t: &mut TestingT) {
    if should_skip_if_failing("TestQualifiedName_import-declaration-with-variable-entity-names") {
        return;
    }
    let content = r#"namespace Alpha {
    export var [|{| "name" : "def" |}x|] = 100;
}

namespace Beta {
    import p = Alpha.[|{| "name" : "import" |}x|];
}

var x = Alpha.[|{| "name" : "mem" |}x|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "import");
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
                    label: "x".to_string(),
                    detail: Some("var Alpha.x: number".to_string()),
                    ..Default::default()
                })],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_baseline_document_highlights(
        t,
        None,
        vec![MarkerOrRangeOrName::Name("import".to_string())],
    );
    f.verify_baseline_go_to_definition(t, &["import".to_string()]);
    done();
}
