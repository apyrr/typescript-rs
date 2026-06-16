#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_class_expression_static() {
    let mut t = TestingT;
    run_test_get_occurrences_class_expression_static(&mut t);
}

fn run_test_get_occurrences_class_expression_static(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"let A = class Foo {
    public [|static|] foo;
    [|static|] a;
    constructor(public y: string, private x: string) {
    }
    public method() { }
    private method2() {}
    public [|static|] static() { }
    private [|static|] static2() { }
}

let B = class D {
    static a;
    constructor(private x: number) {
    }
    private static test() {}
    public static test2() {}
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
