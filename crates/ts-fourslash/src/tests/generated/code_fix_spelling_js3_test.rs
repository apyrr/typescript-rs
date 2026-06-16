#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_spelling_js3() {
    let mut t = TestingT;
    run_test_code_fix_spelling_js3(&mut t);
}

fn run_test_code_fix_spelling_js3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowjs: true
// @noEmit: true
// @filename: a.js
class Classe {
    non = 'oui'
    methode() {
        // no error on 'this' references
        return this.none
    }
}
class Derivee extends Classe {
    methode() {
        // no error on 'super' references
        return super.none
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    done();
}
