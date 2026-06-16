#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_merged_declarations4() {
    let mut t = TestingT;
    run_test_references_for_merged_declarations4(&mut t);
}

fn run_test_references_for_merged_declarations4(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForMergedDeclarations4") {
        return;
    }
    let content = r"/*1*/class /*2*/testClass {
    static staticMethod() { }
    method() { }
}

/*3*/module /*4*/testClass {
    export interface Bar {

    }
    export var s = 0;
}

var c1: /*5*/testClass;
var c2: /*6*/testClass.Bar;
/*7*/testClass.staticMethod();
/*8*/testClass.prototype.method();
/*9*/testClass.bind(this);
/*10*/testClass.s;
new /*11*/testClass();";
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
            "10".to_string(),
            "11".to_string(),
        ],
    );
    done();
}
