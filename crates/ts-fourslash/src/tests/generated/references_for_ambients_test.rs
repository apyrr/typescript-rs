#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_ambients() {
    let mut t = TestingT;
    run_test_references_for_ambients(&mut t);
}

fn run_test_references_for_ambients(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"/*1*/declare module "/*2*/foo" {
    /*3*/var /*4*/f: number;
}

/*5*/declare module "/*6*/bar" {
    /*7*/export import /*8*/foo = require("/*9*/foo");
    var f2: typeof /*10*/foo./*11*/f;
}

declare module "baz" {
    /*12*/import bar = require("/*13*/bar");
    var f2: typeof bar./*14*/foo;
}"#;
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
            "9".to_string(),
            "10".to_string(),
            "11".to_string(),
            "12".to_string(),
            "13".to_string(),
            "14".to_string(),
        ],
    );
    done();
}
