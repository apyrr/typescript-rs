#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_pre_processed_file() {
    let mut t = TestingT;
    run_test_get_pre_processed_file(&mut t);
}

fn run_test_get_pre_processed_file(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @moduleResolution: classic
// @Filename: refFile1.ts
class D { }
// @Filename: refFile2.ts
export class E {}
// @Filename: main.ts
// @ResolveReference: true
///<reference path="refFile1.ts" />
///<reference path = "/*1*/NotExistRef.ts/*2*/" />
/*3*////<reference path "invalidRefFile1.ts" />/*4*/
import ref2 = require("refFile2");
import noExistref2 = require(/*5*/"NotExistRefFile2"/*6*/);
import invalidRef1  /*7*/require/*8*/("refFile2");
import invalidRef2 = /*9*/requi/*10*/(/*10A*/"refFile2");
var obj: /*11*/C/*12*/;
var obj1: D;
var obj2: ref2.E;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "main.ts");
    f.verify_number_of_errors_in_current_file(7);
    f.verify_error_exists_between_markers(&f.marker_by_name("1"), &f.marker_by_name("2"), 0);
    f.verify_error_exists_between_markers(&f.marker_by_name("3"), &f.marker_by_name("4"), 0);
    f.verify_error_exists_between_markers(&f.marker_by_name("5"), &f.marker_by_name("6"), 0);
    f.verify_error_exists_between_markers(&f.marker_by_name("7"), &f.marker_by_name("8"), 0);
    f.verify_error_exists_between_markers(&f.marker_by_name("9"), &f.marker_by_name("10"), 0);
    f.verify_error_exists_between_markers(&f.marker_by_name("10"), &f.marker_by_name("10A"), 0);
    f.verify_error_exists_between_markers(&f.marker_by_name("11"), &f.marker_by_name("12"), 0);
    done();
}
