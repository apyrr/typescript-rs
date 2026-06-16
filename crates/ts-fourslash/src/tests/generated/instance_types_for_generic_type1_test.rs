#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_instance_types_for_generic_type1() {
    let mut t = TestingT;
    run_test_instance_types_for_generic_type1(&mut t);
}

fn run_test_instance_types_for_generic_type1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class G<T> {               // Introduce type parameter T
    self: G<T>;            // Use T as type argument to form instance type
    f() {
        this./*1*/self = /*2*/this;  // self and this are both of type G<T>
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) G<T>.self: G<T>", "");
    f.verify_quick_info_at(t, "2", "this: this", "");
    done();
}
