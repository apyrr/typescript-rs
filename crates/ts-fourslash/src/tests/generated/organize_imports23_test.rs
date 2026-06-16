#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports23() {
    let mut t = TestingT;
    run_test_organize_imports23(&mut t);
}

fn run_test_organize_imports23(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"import {abc, Abc, type bc, type Bc} from 'b';
import {
  I,
  R,
  M,
} from 'a';
type x = bc | Bc;
console.log(abc, Abc, I, R, M);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r"import {
    I,
    M,
    R,
} from 'a';
import { abc, Abc, type bc, type Bc } from 'b';
type x = bc | Bc;
console.log(abc, Abc, I, R, M);",
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
import { abc, Abc, type bc, type Bc } from 'b';
type x = bc | Bc;
console.log(abc, Abc, I, R, M);",
        "source.organizeImports",
        None,
    );
    done();
}
