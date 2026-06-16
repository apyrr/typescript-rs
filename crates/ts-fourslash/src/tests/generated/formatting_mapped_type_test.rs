#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_mapped_type() {
    let mut t = TestingT;
    run_test_formatting_mapped_type(&mut t);
}

fn run_test_formatting_mapped_type(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*generic*/type t  < T  > =   {
/*map*/   [   P   in   keyof    T  ]   :   T  [  P  ]
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "generic");
    f.verify_current_line_content(t, "type t<T> = {");
    f.go_to_marker(t, "map");
    f.verify_current_line_content(t, "    [P in keyof T]: T[P]");
    done();
}
