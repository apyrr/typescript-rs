#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_enums() {
    let mut t = TestingT;
    run_test_references_for_enums(&mut t);
}

fn run_test_references_for_enums(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"enum E {
    /*1*/value1 = 1,
    /*2*/"/*3*/value2" = /*4*/value1,
    /*5*/111 = 11
}

E./*6*/value1;
E["/*7*/value2"];
E./*8*/value2;
E[/*9*/111];"#;
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
        ],
    );
    done();
}
