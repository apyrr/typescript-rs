#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_spelling_js5() {
    let mut t = TestingT;
    run_test_code_fix_spelling_js5(&mut t);
}

fn run_test_code_fix_spelling_js5(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixSpellingJs5") {
        return;
    }
    let content = r"// @allowjs: true
// @noEmit: true
// @filename: a.js
var other = {
    puuce: 4
}
var Jimmy = 1
var John = 2
// @filename: b.js
other.puuuce // OK, from another file
new Date().getGMTDate() // OK, from another file
window.argle // OK, from globalThis
self.blargle // OK, from globalThis

// No suggestions for globals from other files
const atoc = setIntegral(() => console.log('ok'), 500)
AudioBuffin // etc
Jimmy
Jon";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    done();
}
