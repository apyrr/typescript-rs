#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_label6() {
    let mut t = TestingT;
    run_test_rename_label6(&mut t);
}

fn run_test_rename_label6(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"loop1: for (let i = 0; i <= 10; i++) {
    loop2: for (let j = 0; j <= 10; j++) {
        if (i === 5) continue loop1;
        if (j === 5) break /**/loop2;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &["".to_string()]);
    done();
}
