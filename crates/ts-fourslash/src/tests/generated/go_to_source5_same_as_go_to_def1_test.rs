#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_source5_same_as_go_to_def1() {
    let mut t = TestingT;
    run_test_go_to_source5_same_as_go_to_def1(&mut t);
}

fn run_test_go_to_source5_same_as_go_to_def1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
// @Filename: /home/src/workspaces/project/a.ts
export const /*end*/a = 'a';
// @Filename: /home/src/workspaces/project/a.d.ts
export declare const a: string;
// @Filename: /home/src/workspaces/project/a.js
export const a = 'a';
// @Filename: /home/src/workspaces/project/b.ts
import { a } from './a';
[|a/*start*/|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_go_to_source_definition(t, &["start".to_string()]);
    f.verify_baseline_go_to_definition(t, &["start".to_string()]);
    done();
}
