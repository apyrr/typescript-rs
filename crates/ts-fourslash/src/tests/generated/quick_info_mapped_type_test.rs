#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_mapped_type() {
    let mut t = TestingT;
    run_test_quick_info_mapped_type(&mut t);
}

fn run_test_quick_info_mapped_type(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface I {
  /** m documentation */ m(): void;
}
declare const o: { [K in keyof I]: number };
o.m/*0*/;

declare const p: { [K in keyof I]: I[K] };
p.m/*1*/;

declare const q: Pick<I, "m">;
q.m/*2*/;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "0", "(property) m: number", "m documentation");
    f.verify_quick_info_at(t, "1", "(method) m(): void", "m documentation");
    f.verify_quick_info_at(t, "2", "(method) m(): void", "m documentation");
    done();
}
