#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_default_export02() {
    let mut t = TestingT;
    run_test_find_all_refs_for_default_export02(&mut t);
}

fn run_test_find_all_refs_for_default_export02(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsForDefaultExport02") {
        return;
    }
    let content = r"/*1*/export default function /*2*/DefaultExportedFunction() {
    return /*3*/DefaultExportedFunction;
}

var x: typeof /*4*/DefaultExportedFunction;

var y = /*5*/DefaultExportedFunction();

/*6*/namespace /*7*/DefaultExportedFunction {
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
        ],
    );
    done();
}
