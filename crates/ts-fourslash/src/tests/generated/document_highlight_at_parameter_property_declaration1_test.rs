#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_document_highlight_at_parameter_property_declaration1() {
    let mut t = TestingT;
    run_test_document_highlight_at_parameter_property_declaration1(&mut t);
}

fn run_test_document_highlight_at_parameter_property_declaration1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: file1.ts
class Foo {
    constructor(private [|privateParam|]: number,
        public [|publicParam|]: string,
        protected [|protectedParam|]: boolean) {

        let localPrivate = [|privateParam|];
        this.[|privateParam|] += 10;

        let localPublic = [|publicParam|];
        this.[|publicParam|] += " Hello!";

        let localProtected = [|protectedParam|];
        this.[|protectedParam|] = false;
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        f.ranges()
            .into_iter()
            .map(MarkerOrRangeOrName::Range)
            .collect(),
    );
    done();
}
