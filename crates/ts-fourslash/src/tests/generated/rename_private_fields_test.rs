#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_private_fields() {
    let mut t = TestingT;
    run_test_rename_private_fields(&mut t);
}

fn run_test_rename_private_fields(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Foo {
   [|/**/#foo|] = 1;

   getFoo() {
       return this.#foo;
   }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_rename_succeeded_at_current_position();
    done();
}
