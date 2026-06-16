#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_incremental_resolve_accessor() {
    let mut t = TestingT;
    run_test_incremental_resolve_accessor(&mut t);
}

fn run_test_incremental_resolve_accessor(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class c1 {
    get p1(): string {
        return "30";
    }
    set p1(a: number) {
        a = "30";
    }
}
var val = new c1();
var b = val.p1;
/*1*/b;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var b: string", "");
    f.verify_number_of_errors_in_current_file(1);
    done();
}
