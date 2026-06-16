#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_iterator_next() {
    let mut t = TestingT;
    run_test_signature_help_iterator_next(&mut t);
}

fn run_test_signature_help_iterator_next(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: esnext
declare const iterator: Iterator<string, void, number>;

iterator.next(/*1*/);
iterator.next(/*2*/ 0);

declare const generator: Generator<string, void, number>;

generator.next(/*3*/);
generator.next(/*4*/ 0);

declare const asyncIterator: AsyncIterator<string, void, number>;

asyncIterator.next(/*5*/);
asyncIterator.next(/*6*/ 0);

declare const asyncGenerator: AsyncGenerator<string, void, number>;

asyncGenerator.next(/*7*/);
asyncGenerator.next(/*8*/ 0);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
