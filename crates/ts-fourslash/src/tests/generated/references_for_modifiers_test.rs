#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_modifiers() {
    let mut t = TestingT;
    run_test_references_for_modifiers(&mut t);
}

fn run_test_references_for_modifiers(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
[|/*declareModifier*/declare /*abstractModifier*/abstract class C1 {
    [|/*staticModifier*/static a;|]
    [|/*readonlyModifier*/readonly b;|]
    [|/*publicModifier*/public c;|]
    [|/*protectedModifier*/protected d;|]
    [|/*privateModifier*/private e;|]
}|]
[|/*constModifier*/const enum E {
}|]
[|/*asyncModifier*/async function fn() {}|]
[|/*exportModifier*/export /*defaultModifier*/default class C2 {}|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "declareModifier".to_string(),
            "abstractModifier".to_string(),
            "staticModifier".to_string(),
            "readonlyModifier".to_string(),
            "publicModifier".to_string(),
            "protectedModifier".to_string(),
            "privateModifier".to_string(),
            "constModifier".to_string(),
            "asyncModifier".to_string(),
            "exportModifier".to_string(),
            "defaultModifier".to_string(),
        ],
    );
    done();
}
