#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_aliased_generic() {
    let mut t = TestingT;
    run_test_quick_info_for_aliased_generic(&mut t);
}

fn run_test_quick_info_for_aliased_generic(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace M {
    export namespace N {
        export class C<T> { }
        export class D { }
    }
}
import d = M.N;
var /*1*/aa: d.C<number>;
var /*2*/bb: d.D;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var aa: d.C<number>", "");
    f.verify_quick_info_at(t, "2", "var bb: d.D", "");
    done();
}
