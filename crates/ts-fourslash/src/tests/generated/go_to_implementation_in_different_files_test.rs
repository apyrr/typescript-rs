#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_in_different_files() {
    let mut t = TestingT;
    run_test_go_to_implementation_in_different_files(&mut t);
}

fn run_test_go_to_implementation_in_different_files(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
// @Filename: /home/src/workspaces/project/bar.ts
import {Foo} from './foo'

class [|A|] implements Foo {
    func() {}
}

class [|B|] implements Foo {
    func() {}
}
// @Filename: /home/src/workspaces/project/foo.ts
export interface /**/Foo {
    func();
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_go_to_implementation(t, &["".to_string()]);
    done();
}
