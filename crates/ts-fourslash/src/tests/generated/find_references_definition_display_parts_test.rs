#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_references_definition_display_parts() {
    let mut t = TestingT;
    run_test_find_references_definition_display_parts(&mut t);
}

fn run_test_find_references_definition_display_parts(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class Gre/*1*/eter {
    someFunction() { th/*2*/is;  }
}

type Options = "opt/*3*/ion 1" | "option 2";
let myOption: Options = "option 1";

some/*4*/Label:
break someLabel;"#;
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
