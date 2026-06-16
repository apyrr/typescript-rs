#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_static() {
    let mut t = TestingT;
    run_test_references_for_static(&mut t);
}

fn run_test_references_for_static(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: referencesOnStatic_1.ts
var n = 43;

class foo {
    /*1*/static /*2*/n = '';

    public bar() {
        foo./*3*/n = "'";
        if(foo./*4*/n) {
            var x = foo./*5*/n;
        }
    }
}

class foo2 {
    private x = foo./*6*/n;
    constructor() {
        foo./*7*/n = x;
    }

    function b(n) {
        n = foo./*8*/n;
    }
}
// @Filename: referencesOnStatic_2.ts
var q = foo./*9*/n;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
            "8".to_string(),
            "9".to_string(),
        ],
    );
    done();
}
