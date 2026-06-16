#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_type_argument_on_new_line() {
    let mut t = TestingT;
    run_test_format_type_argument_on_new_line(&mut t);
}

fn run_test_format_type_argument_on_new_line(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"const genericObject = new GenericObject<
  /*1*/{}
>();
const genericObject2 = new GenericObject2<
  /*2*/{},
  /*3*/{}
>();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "    {}");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    {},");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    {}");
    done();
}
