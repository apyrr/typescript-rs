#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_document_highlight_template_strings() {
    let mut t = TestingT;
    run_test_document_highlight_template_strings(&mut t);
}

fn run_test_document_highlight_template_strings(t: &mut TestingT) {
    if should_skip_if_failing("TestDocumentHighlightTemplateStrings") {
        return;
    }
    let content = r#"type Foo = "[|a|]" | "b";

class C {
   p: Foo = `[|a|]`;
   m() {
       switch (this.p) {
           case `[|a|]`:
               return 1;
           case "b":
               return 2;
       }
   }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        vec![MarkerOrRangeOrName::Range(f.ranges()[2].clone())],
    );
    done();
}
