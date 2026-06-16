#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports3() {
    let mut t = TestingT;
    run_test_organize_imports3(&mut t);
}

fn run_test_organize_imports3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"import {
    Bar   
    , Foo   
  } from "foo"

console.log(Foo, Bar);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"import {
    Bar,
    Foo
} from "foo";

console.log(Foo, Bar);"#,
        "source.organizeImports",
        None,
    );
    done();
}
