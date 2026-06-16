#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_parameter_with_destructuring() {
    let mut t = TestingT;
    run_test_parameter_with_destructuring(&mut t);
}

fn run_test_parameter_with_destructuring(t: &mut TestingT) {
    if should_skip_if_failing("TestParameterWithDestructuring") {
        return;
    }
    let content = r"const result = [{ a: 'hello' }]
    .map(({ /*1*/a }) => /*2*/a)
    .map(a => a);

const f1 = (a: (b: string[]) => void) => {};
f1(([a, b]) => { /*3*/a.charAt(0); });

function f2({/*4*/a }: { a: string; }, [/*5*/b]: [string]) {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(parameter) a: string", "");
    f.verify_quick_info_at(t, "2", "(parameter) a: string", "");
    f.verify_quick_info_at(t, "3", "(parameter) a: string", "");
    f.verify_quick_info_at(t, "4", "(parameter) a: string", "");
    f.verify_quick_info_at(t, "5", "(parameter) b: string", "");
    done();
}
