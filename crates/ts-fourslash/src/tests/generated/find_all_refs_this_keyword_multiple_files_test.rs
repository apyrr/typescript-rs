#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_this_keyword_multiple_files() {
    let mut t = TestingT;
    run_test_find_all_refs_this_keyword_multiple_files(&mut t);
}

fn run_test_find_all_refs_this_keyword_multiple_files(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsThisKeywordMultipleFiles") {
        return;
    }
    let content = r"// @Filename: file1.ts
/*1*/this; /*2*/this;
// @Filename: file2.ts
/*3*/this;
/*4*/this;
// @Filename: file3.ts
 ((x = /*5*/this, y) => /*6*/this)(/*7*/this, /*8*/this);
 // different 'this'
 function f(this) { return this; }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
            "8".to_string(),
        ],
    );
    done();
}
