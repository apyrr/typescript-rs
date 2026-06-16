#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_async_await() {
    let mut t = TestingT;
    run_test_get_occurrences_async_await(&mut t);
}

fn run_test_get_occurrences_async_await(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesAsyncAwait") {
        return;
    }
    let content = r"[|async|] function f() {
 [|await|] 100;
 [|a/**/wait|] [|await|] 200;
class Foo {
    async memberFunction() {
        await 1;
    }
}
 return [|await|] async function () {
   await 300;
 }
}
async function g() {
    await 300;
    async function f() {
        await 400;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        f.ranges()
            .into_iter()
            .map(MarkerOrRangeOrName::Range)
            .collect(),
    );
    done();
}
