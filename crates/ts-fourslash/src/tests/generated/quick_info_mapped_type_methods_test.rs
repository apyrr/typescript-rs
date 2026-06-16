#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_mapped_type_methods() {
    let mut t = TestingT;
    run_test_quick_info_mapped_type_methods(&mut t);
}

fn run_test_quick_info_mapped_type_methods(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"type M = { [K in 'one']: any };
const x: M = {
  /**/one() {}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(property) one: any", "");
    done();
}
