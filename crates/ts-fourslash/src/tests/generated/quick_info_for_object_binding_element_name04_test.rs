#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_object_binding_element_name04() {
    let mut t = TestingT;
    run_test_quick_info_for_object_binding_element_name04(&mut t);
}

fn run_test_quick_info_for_object_binding_element_name04(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Options {
   /**
    * A description of 'a'
    */
    a: {
       /**
        * A description of 'b'
        */
       b: string;
   }
}

function f({ a, a: { b } }: Options) {
    a/*1*/;
    b/*2*/;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
