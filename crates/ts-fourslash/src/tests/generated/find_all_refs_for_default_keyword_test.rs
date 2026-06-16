#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_default_keyword() {
    let mut t = TestingT;
    run_test_find_all_refs_for_default_keyword(&mut t);
}

fn run_test_find_all_refs_for_default_keyword(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @noLib: true
function f(value: string, /*1*/default: string) {}

const /*2*/default = 1;

function /*3*/default() {}

class /*4*/default {}

const foo = {
    /*5*/default: 1
}";
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
