#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_namespace() {
    let mut t = TestingT;
    run_test_rename_namespace(&mut t);
}

fn run_test_rename_namespace(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameNamespace") {
        return;
    }
    let content = r"namespace /**/NS {
    export const enum E {
        A = 'a'
    }
}

const a: NS.E = NS.E.A;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &["".to_string()]);
    done();
}
