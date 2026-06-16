#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_inside_templates2() {
    let mut t = TestingT;
    run_test_find_all_refs_inside_templates2(&mut t);
}

fn run_test_find_all_refs_inside_templates2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"/*1*/function /*2*/f(...rest: any[]) { }
/*3*/f ` + "`" + `${ /*4*/f } ${ /*5*/f }` + "`" + `"#;
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
