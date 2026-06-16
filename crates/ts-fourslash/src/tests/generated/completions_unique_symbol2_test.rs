#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_unique_symbol2() {
    let mut t = TestingT;
    run_test_completions_unique_symbol2(&mut t);
}

fn run_test_completions_unique_symbol2(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsUniqueSymbol2") {
        return;
    }
    let content = r"const a = {
    KEY_1: 'key_1',
    KEY_2: 'key_2',
    KEY_3: 'key_3',
} as const;

const b = {
    KEY_1: 'key_1',
    KEY_2: 'key_2',
    KEY_3: 'key_3',
} as const;

interface I {
    [b.KEY_1]: string,
    [a.KEY_2]: string,
    [a.KEY_3]: string
}

const foo: I = {
    key_1: 'value_1',
    key_2: 'value_2',
    key_3: 'value_3',
}

foo./**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
