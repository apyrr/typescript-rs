#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_process_invalid_syntax1() {
    let mut t = TestingT;
    run_test_process_invalid_syntax1(&mut t);
}

fn run_test_process_invalid_syntax1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowJs: true
// @Filename: decl.js
var obj = {};
// @Filename: unicode1.js
obj.𝒜 ;
// @Filename: unicode2.js
obj.¬ ;
// @Filename: unicode3.js
obj¬
// @Filename: forof.js
for (obj/**/.prop of arr) {

}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &["".to_string()]);
    done();
}
