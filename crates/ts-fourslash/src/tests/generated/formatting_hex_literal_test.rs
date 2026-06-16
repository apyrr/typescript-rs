#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_hex_literal() {
    let mut t = TestingT;
    run_test_formatting_hex_literal(&mut t);
}

fn run_test_formatting_hex_literal(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var x =  0x1,y;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    done();
}
