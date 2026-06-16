#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_outline_spans_block_comments_without_statements() {
    let mut t = TestingT;
    run_test_outline_spans_block_comments_without_statements(&mut t);
}

fn run_test_outline_spans_block_comments_without_statements(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"[|/*
/ * Some text
  */|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
