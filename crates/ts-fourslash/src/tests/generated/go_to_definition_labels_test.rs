#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_labels() {
    let mut t = TestingT;
    run_test_go_to_definition_labels(&mut t);
}

fn run_test_go_to_definition_labels(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*label1Definition*/label1: while (true) {
    /*label2Definition*/label2: while (true) {
        break [|/*1*/label1|];
        continue [|/*2*/label2|];
        () => { break [|/*3*/label1|]; }
        continue /*4*/unknownLabel;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
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
