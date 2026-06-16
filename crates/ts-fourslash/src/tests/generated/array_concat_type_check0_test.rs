#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_array_concat_type_check0() {
    let mut t = TestingT;
    run_test_array_concat_type_check0(&mut t);
}

fn run_test_array_concat_type_check0(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"var a = [];
a.concat("hello"/*1*/);

a.concat('Hello');

var b = new Array();
b.concat('hello');
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.disable_formatting();
    f.go_to_marker(t, "1");
    f.insert(t, ", 'world'");
    done();
}
