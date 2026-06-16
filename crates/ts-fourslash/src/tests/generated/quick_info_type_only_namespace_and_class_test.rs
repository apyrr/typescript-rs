#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_type_only_namespace_and_class() {
    let mut t = TestingT;
    run_test_quick_info_type_only_namespace_and_class(&mut t);
}

fn run_test_quick_info_type_only_namespace_and_class(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: /a.ts
export namespace ns {
  export class Box<T> {}
}
// @Filename: /b.ts
import type { ns } from './a';
let x: /*1*/ns./*2*/Box<string>;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(alias) namespace ns\nimport ns", "");
    f.verify_quick_info_at(t, "2", "class ns.Box<T>", "");
    done();
}
