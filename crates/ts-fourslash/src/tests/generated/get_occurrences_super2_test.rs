#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_super2() {
    let mut t = TestingT;
    run_test_get_occurrences_super2(&mut t);
}

fn run_test_get_occurrences_super2(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesSuper2") {
        return;
    }
    let content = r"class SuperType {
    superMethod() {
    }

    static superStaticMethod() {
        return 10;
    }
}

class SubType extends SuperType {
    public  prop1 = super.superMethod;
    private prop2 = super.superMethod;

    constructor() {
        super();
    }

    public method1() {
        return super.superMethod();
    }

    private method2() {
        return super.superMethod();
    }

    public method3() {
        var x = () => super.superMethod();

        // Bad but still gets highlighted
        function f() {
            super.superMethod();
        }
    }

    // Bad but still gets highlighted.
    public static statProp1 = [|super|].superStaticMethod;

    public static staticMethod1() {
        return [|super|].superStaticMethod();
    }

    private static staticMethod2() {
        return [|supe/**/r|].superStaticMethod();
    }

    // Are not actually 'super' keywords.
    super = 10;
    static super = 20;
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
