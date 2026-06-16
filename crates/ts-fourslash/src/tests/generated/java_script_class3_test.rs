#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_java_script_class3() {
    let mut t = TestingT;
    run_test_java_script_class3(&mut t);
}

fn run_test_java_script_class3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowNonTsExtensions: true
// @Filename: Foo.js
class Foo {
   constructor() {
       this./*dst1*/alpha = 10;
       this./*dst2*/beta = 'gamma';
   }
   method() { return this.alpha; }
}
var x = new Foo();
x.[|alpha/*src1*/|];
x.[|beta/*src2*/|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["src1".to_string(), "src2".to_string()]);
    done();
}
