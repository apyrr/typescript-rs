#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_default_export03() {
    let mut t = TestingT;
    run_test_find_all_refs_for_default_export03(&mut t);
}

fn run_test_find_all_refs_for_default_export03(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*1*/function /*2*/f() {
    return 100;
}

/*3*/export default /*4*/f;

var x: typeof /*5*/f;

var y = /*6*/f();

/*7*/namespace /*8*/f {
    var local = 100;
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
            "6".to_string(),
            "7".to_string(),
            "8".to_string(),
        ],
    );
    done();
}
