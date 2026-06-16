#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_class_expression_public() {
    let mut t = TestingT;
    run_test_get_occurrences_class_expression_public(&mut t);
}

fn run_test_get_occurrences_class_expression_public(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"let A = class Foo {
    [|public|] foo;
    [|public|] public;
    constructor([|public|] y: string, private x: string) {
    }
    [|public|] method() { }
    private method2() {}
    [|public|] static static() { }
}

let B = class D {
    constructor(private x: number) {
    }
    private test() {}
    public test2() {}
}";
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
