#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_merged_declarations5() {
    let mut t = TestingT;
    run_test_references_for_merged_declarations5(&mut t);
}

fn run_test_references_for_merged_declarations5(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface /*1*/Foo { }
module /*2*/Foo { export interface Bar { } }
function /*3*/Foo() { }

export = /*4*/Foo;";
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
