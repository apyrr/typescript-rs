#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports1() {
    let mut t = TestingT;
    run_test_organize_imports1(&mut t);
}

fn run_test_organize_imports1(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImports1") {
        return;
    }
    let content = r"import {
    d, d as D,
    c,
    c as C, b,
    b as B, a
} from './foo';
import {
    h, h as H,
    g,
    g as G, f,
    f as F, e
} from './foo';

console.log(a, B, b, c, C, d, D);
console.log(e, f, F, g, G, H, h);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r"import {
    a,
    b,
    b as B,
    c,
    c as C,
    d, d as D,
    e,
    f,
    f as F,
    g,
    g as G,
    h, h as H
} from './foo';

console.log(a, B, b, c, C, d, D);
console.log(e, f, F, g, G, H, h);",
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSTrue,
            ..Default::default()
        }),
    );
    f.verify_organize_imports(
        t,
        r"import {
    b as B,
    c as C,
    d as D,
    f as F,
    g as G,
    h as H,
    a,
    b,
    c,
    d,
    e,
    f,
    g,
    h
} from './foo';

console.log(a, B, b, c, C, d, D);
console.log(e, f, F, g, G, H, h);",
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSFalse,
            ..Default::default()
        }),
    );
    done();
}
