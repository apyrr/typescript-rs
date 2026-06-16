#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_globals_in_external_module() {
    let mut t = TestingT;
    run_test_references_for_globals_in_external_module(&mut t);
}

fn run_test_references_for_globals_in_external_module(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForGlobalsInExternalModule") {
        return;
    }
    let content = r"/*1*/var /*2*/topLevelVar = 2;
var topLevelVar2 = /*3*/topLevelVar;

/*4*/class /*5*/topLevelClass { }
var c = new /*6*/topLevelClass();

/*7*/interface /*8*/topLevelInterface { }
var i: /*9*/topLevelInterface;

/*10*/module /*11*/topLevelModule {
    export var x;
}
var x = /*12*/topLevelModule.x;

export = x;";
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
            "12".to_string(),
        ],
    );
    done();
}
