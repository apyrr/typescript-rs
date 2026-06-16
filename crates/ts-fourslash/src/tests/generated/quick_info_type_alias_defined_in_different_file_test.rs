#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_type_alias_defined_in_different_file() {
    let mut t = TestingT;
    run_test_quick_info_type_alias_defined_in_different_file(&mut t);
}

fn run_test_quick_info_type_alias_defined_in_different_file(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /a.ts
export type X = { x: number };
export function f(x: X): void {}
// @Filename: /b.ts
import { f } from "./a";
/**/f({ x: 1 });"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(alias) f(x: X): void\nimport f", "");
    done();
}
