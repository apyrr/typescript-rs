#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_type_parameter_in_function_like_in_type_alias() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_type_parameter_in_function_like_in_type_alias(&mut t);
}

fn run_test_quick_info_display_parts_type_parameter_in_function_like_in_type_alias(
    t: &mut TestingT,
) {
    if should_skip_if_failing("TestQuickInfoDisplayPartsTypeParameterInFunctionLikeInTypeAlias") {
        return;
    }
    let content = r"type MixinCtor<A> = new () => /*0*/A & { constructor: MixinCtor</*1*/A> };
type MixinCtor<A> = new () => A & { constructor: { constructor: MixinCtor</*2*/A> } };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
