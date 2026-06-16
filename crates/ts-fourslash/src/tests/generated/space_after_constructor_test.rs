#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_space_after_constructor() {
    let mut t = TestingT;
    run_test_space_after_constructor(&mut t);
}

fn run_test_space_after_constructor(t: &mut TestingT) {
    if should_skip_if_failing("TestSpaceAfterConstructor") {
        return;
    }
    let content = r"export class myController {
    private _processId;
    constructor (processId: number) {/*1*/
        this._processId = processId;
    }/*2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "2");
    f.insert(t, "}");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "    constructor(processId: number) {");
    done();
}
