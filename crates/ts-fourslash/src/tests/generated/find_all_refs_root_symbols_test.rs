#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_root_symbols() {
    let mut t = TestingT;
    run_test_find_all_refs_root_symbols(&mut t);
}

fn run_test_find_all_refs_root_symbols(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I { /*0*/x: {}; }
interface J { /*1*/x: {}; }
declare const o: (I | J) & { /*2*/x: string };
o./*3*/x;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "0".to_string(),
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
        ],
    );
    done();
}
