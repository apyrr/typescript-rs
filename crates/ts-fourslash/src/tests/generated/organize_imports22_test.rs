#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports22() {
    let mut t = TestingT;
    run_test_organize_imports22(&mut t);
}

fn run_test_organize_imports22(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"import {abc, Abc, bc, Bc} from 'b';
import {
  I,
  R,
  M,
} from 'a';
console.log(abc, Abc, bc, Bc, I, R, M);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r"import {
    I,
    M,
    R,
} from 'a';
import { abc, Abc, bc, Bc } from 'b';
console.log(abc, Abc, bc, Bc, I, R, M);",
        "source.organizeImports",
        None,
    );
    f.verify_organize_imports(
        t,
        r"import {
    I,
    M,
    R,
} from 'a';
import { abc, Abc, bc, Bc } from 'b';
console.log(abc, Abc, bc, Bc, I, R, M);",
        "source.organizeImports",
        None,
    );
    done();
}
