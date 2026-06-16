#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_class_members() {
    let mut t = TestingT;
    run_test_references_for_class_members(&mut t);
}

fn run_test_references_for_class_members(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Base {
    /*a1*/a: number;
    /*method1*/method(): void { }
}
class MyClass extends Base {
    /*a2*/a;
    /*method2*/method() { }
}

var c: MyClass;
c./*a3*/a;
c./*method3*/method();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "a1".to_string(),
            "a2".to_string(),
            "a3".to_string(),
            "method1".to_string(),
            "method2".to_string(),
            "method3".to_string(),
        ],
    );
    done();
}
