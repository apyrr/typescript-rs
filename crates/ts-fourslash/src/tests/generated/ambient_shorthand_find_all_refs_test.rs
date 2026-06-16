#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_ambient_shorthand_find_all_refs() {
    let mut t = TestingT;
    run_test_ambient_shorthand_find_all_refs(&mut t);
}

fn run_test_ambient_shorthand_find_all_refs(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: declarations.d.ts
declare module "jquery";
// @Filename: user.ts
import {/*1*/x} from "jquery";
// @Filename: user2.ts
import {/*2*/x} from "jquery";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}
