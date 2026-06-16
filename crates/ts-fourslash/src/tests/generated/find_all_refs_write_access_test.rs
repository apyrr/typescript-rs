#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_write_access() {
    let mut t = TestingT;
    run_test_find_all_refs_write_access(&mut t);
}

fn run_test_find_all_refs_write_access(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface Obj {
    [` + "`" + `/*1*/num` + "`" + `]: number;
}

let o: Obj = {
    [` + "`" + `num` + "`" + `]: 0
};

o = {
    ['num']: 1
};

o['num'] = 2;
o[` + "`" + `num` + "`" + `] = 3;

o['num'];
o[` + "`" + `num` + "`" + `];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
