#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_is_definition_of_interface_class_merge() {
    let mut t = TestingT;
    run_test_get_occurrences_is_definition_of_interface_class_merge(&mut t);
}

fn run_test_get_occurrences_is_definition_of_interface_class_merge(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesIsDefinitionOfInterfaceClassMerge") {
        return;
    }
    let content = r"/*1*/interface /*2*/Numbers {
    p: number;
}
/*3*/interface /*4*/Numbers {
    m: number;
}
/*5*/class /*6*/Numbers {
    f(n: number) {
        return this.p + this.m + n;
    }
}
let i: /*7*/Numbers = new /*8*/Numbers();
let x = i.f(i.p + i.m);";
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
        ],
    );
    done();
}
