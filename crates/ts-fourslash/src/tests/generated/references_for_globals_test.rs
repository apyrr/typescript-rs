#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_globals() {
    let mut t = TestingT;
    run_test_references_for_globals(&mut t);
}

fn run_test_references_for_globals(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: referencesForGlobals_1.ts
/*1*/var /*2*/global = 2;

class foo {
    constructor (public global) { }
    public f(global) { }
    public f2(global) { }
}

class bar {
    constructor () {
        var n = /*3*/global;

        var f = new foo('');
        f.global = '';
    }
}

var k = /*4*/global;
// @Filename: referencesForGlobals_2.ts
var m = /*5*/global;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
        ],
    );
    done();
}
