#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_09() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_09(&mut t);
}

fn run_test_go_to_implementation_interface_09(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: def.d.ts
export interface Interface { P: number }
// @Filename: ref.ts
import { Interface } from "./def";
const c: I/*ref*/nterface = [|{ P: 2 }|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["ref".to_string()]);
    done();
}
