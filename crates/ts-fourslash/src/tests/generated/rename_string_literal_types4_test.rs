#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_string_literal_types4() {
    let mut t = TestingT;
    run_test_rename_string_literal_types4(&mut t);
}

fn run_test_rename_string_literal_types4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface I {
    "Prop 1": string;
}

declare const fn: <K extends keyof I>(p: K) => void

fn("Prop 1"/**/)"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &["".to_string()]);
    done();
}
