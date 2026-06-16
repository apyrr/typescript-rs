#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_link3() {
    let mut t = TestingT;
    run_test_jsdoc_link3(&mut t);
}

fn run_test_jsdoc_link3(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocLink3") {
        return;
    }
    let content = r"// @Filename: /jsdocLink3.ts
export class C {
}
// @Filename: /module1.ts
import { C } from './jsdocLink3'
/**
 * {@link C}
 * @wat Makes a {@link C}. A default one.
 * {@link C()}
 * {@link C|postfix text}
 * {@link unformatted postfix text}
 * @see {@link C} its great
 */
function /**/CC() {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
