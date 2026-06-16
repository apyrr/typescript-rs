#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_type_keywords() {
    let mut t = TestingT;
    run_test_references_for_type_keywords(&mut t);
}

fn run_test_references_for_type_keywords(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I {}
function f<T /*typeParam_extendsKeyword*/extends I>() {}
type A1<T, U> = T /*conditionalType_extendsKeyword*/extends U ? 1 : 0;
type A2<T> = T extends /*inferType_inferKeyword*/infer U ? 1 : 0;
type A3<T> = { [P /*mappedType_inOperator*/in keyof T]: 1 };
type A4<T> = /*keyofOperator_keyofKeyword*/keyof T;
type A5<T> = /*readonlyOperator_readonlyKeyword*/readonly T[];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "typeParam_extendsKeyword".to_string(),
            "conditionalType_extendsKeyword".to_string(),
            "inferType_inferKeyword".to_string(),
            "mappedType_inOperator".to_string(),
            "keyofOperator_keyofKeyword".to_string(),
            "readonlyOperator_readonlyKeyword".to_string(),
        ],
    );
    done();
}
