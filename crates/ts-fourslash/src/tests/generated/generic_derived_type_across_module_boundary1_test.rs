#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_derived_type_across_module_boundary1() {
    let mut t = TestingT;
    run_test_generic_derived_type_across_module_boundary1(&mut t);
}

fn run_test_generic_derived_type_across_module_boundary1(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericDerivedTypeAcrossModuleBoundary1") {
        return;
    }
    let content = r"namespace M {
   export class C1 { }
   export class C2<T> { }
}
var c = new M.C2<number>();
namespace N {
   export class D1 extends M.C1 { }
   export class D2<T> extends M.C2<T> { }
}
var n = new N.D1();
var /*1*/n2 = new N.D2<number>();
var /*2*/n3 = new N.D2();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var n2: N.D2<number>", "");
    f.verify_quick_info_at(t, "2", "var n3: N.D2<unknown>", "");
    done();
}
