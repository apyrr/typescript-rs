#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_contextually_typed_function_in_return_statement() {
    let mut t = TestingT;
    run_test_quick_info_for_contextually_typed_function_in_return_statement(&mut t);
}

fn run_test_quick_info_for_contextually_typed_function_in_return_statement(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForContextuallyTypedFunctionInReturnStatement") {
        return;
    }
    let content = r"interface Accumulator {
    clear(): void;
    add(x: number): void;
    result(): number;
}

function makeAccumulator(): Accumulator {
    var sum = 0;
    return {
        clear: function () { sum = 0; },
        add: function (val/**/ue) { sum += value; },
        result: function () { return sum; }
    };
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(parameter) value: number", "");
    done();
}
