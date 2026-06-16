#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_space_after_implements_extends() {
    let mut t = TestingT;
    run_test_format_space_after_implements_extends(&mut t);
}

fn run_test_format_space_after_implements_extends(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C1 implements Array<string>{
}

class C2 implements Number{
}

class C3 extends Array<string>{
}

class C4 extends Number{
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"class C1 implements Array<string> {
}

class C2 implements Number {
}

class C3 extends Array<string> {
}

class C4 extends Number {
}",
    );
    done();
}
