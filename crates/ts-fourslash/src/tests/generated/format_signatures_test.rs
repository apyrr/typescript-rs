#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_signatures() {
    let mut t = TestingT;
    run_test_format_signatures(&mut t);
}

fn run_test_format_signatures(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"type Foo = {
    (
      call: any/*callAutoformat*/
/*callIndent*/
    ): void;
    new (
    constr: any/*constrAutoformat*/
/*constrIndent*/
    ): void;
    method(
       whatever: any/*methodAutoformat*/
/*methodIndent*/
    ): void;
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "callAutoformat");
    f.verify_current_line_content(t, "        call: any");
    f.go_to_marker(t, "callIndent");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "constrAutoformat");
    f.verify_current_line_content(t, "        constr: any");
    f.go_to_marker(t, "constrIndent");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "methodAutoformat");
    f.verify_current_line_content(t, "        whatever: any");
    f.go_to_marker(t, "methodIndent");
    f.verify_indentation(t, 8);
    done();
}
