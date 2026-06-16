#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_super_in_derived_type_of_generic_with_statics() {
    let mut t = TestingT;
    run_test_super_in_derived_type_of_generic_with_statics(&mut t);
}

fn run_test_super_in_derived_type_of_generic_with_statics(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
namespace M {
   export class C<T extends Date> {
      static foo(): C<Date> {
          return null;
           }
     }
}
class D extends M.C<Date> {
    constructor() {
        /**/ // was an error appearing on super in editing scenarios
       }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "super();");
    f.verify_no_errors();
    done();
}
