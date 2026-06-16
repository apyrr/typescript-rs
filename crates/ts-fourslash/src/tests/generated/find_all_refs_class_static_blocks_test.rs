#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_class_static_blocks() {
    let mut t = TestingT;
    run_test_find_all_refs_class_static_blocks(&mut t);
}

fn run_test_find_all_refs_class_static_blocks(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class ClassStaticBocks {
    static x;
    [|[|/*classStaticBocks1*/static|] {}|]
    static y;
    [|[|/*classStaticBocks2*/static|] {}|]
    static y;
    [|[|/*classStaticBocks3*/static|] {}|]
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "classStaticBocks1".to_string(),
            "classStaticBocks2".to_string(),
            "classStaticBocks3".to_string(),
        ],
    );
    done();
}
