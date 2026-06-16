#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_merged_declarations2() {
    let mut t = TestingT;
    run_test_references_for_merged_declarations2(&mut t);
}

fn run_test_references_for_merged_declarations2(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForMergedDeclarations2") {
        return;
    }
    let content = r"namespace ATest {
    export interface Bar { }
}

function ATest() { }

/*1*/import /*2*/alias = ATest; // definition

var a: /*3*/alias.Bar; // namespace
/*4*/alias.call(this); // value";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
    );
    done();
}
