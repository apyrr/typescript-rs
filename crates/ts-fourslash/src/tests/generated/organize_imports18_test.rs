#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports18() {
    let mut t = TestingT;
    run_test_organize_imports18(&mut t);
}

fn run_test_organize_imports18(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImports18") {
        return;
    }
    let content = r#"// @filename: /A.ts
export interface A {}
export function bFuncA(a: A) {}
// @filename: /B.ts
export interface B {}
export function bFuncB(b: B) {}
// @filename: /C.ts
export interface C {}
export function bFuncC(c: C) {}
// @filename: /test.ts
export { C } from "./C";
export { B } from "./B";
export { A } from "./A";

export { bFuncC } from "./C";
export { bFuncB } from "./B";
export { bFuncA } from "./A";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/test.ts");
    f.verify_organize_imports(
        t,
        r#"export { A } from "./A";
export { B } from "./B";
export { C } from "./C";

export { bFuncA } from "./A";
export { bFuncB } from "./B";
export { bFuncC } from "./C";
"#,
        "source.organizeImports",
        None,
    );
    done();
}
