#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_interactive_multifile1() {
    let mut t = TestingT;
    run_test_inlay_hints_interactive_multifile1(&mut t);
}

fn run_test_inlay_hints_interactive_multifile1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
// @Filename: /a.ts
export interface Foo { a: string }
// @Filename: /b.ts
async function foo () {
    return {} as any as import('./a').Foo
}
function bar () { return import('./a') }
async function main () {
    const a = await foo()
    const b = await bar()
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_baseline_inlay_hints(t);
    done();
}
