#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_all() {
    let mut t = TestingT;
    run_test_import_name_code_fix_all(&mut t);
}

fn run_test_import_name_code_fix_all(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @module: commonjs
// @esModuleInterop: false
// @allowSyntheticDefaultImports: false
// @Filename: /a.ts
export default function ad() {}
export const a0 = 0;
// @Filename: /b.ts
export default function bd() {}
export const b0 = 0;
// @Filename: /c.ts
export default function cd() {}
export const c0 = 0;
// @Filename: /d.ts
export default function dd() {}
export const d0 = 0;
export const d1 = 1;
// @Filename: /e.d.ts
declare function e(): void;
export = e;
// @Filename: /disposable.d.ts
export declare class Disposable { }
// @Filename: /disposable_global.d.ts
interface Disposable { }
// @Filename: /user.ts
import * as b from "./b";
import { } from "./c";
import dd from "./d";

ad; ad; a0; a0;
bd; bd; b0; b0;
cd; cd; c0; c0;
dd; dd; d0; d0; d1; d1;
e; e;
class X extends Disposable { }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/user.ts");
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingImport".to_string(),
            new_file_content: r#"import ad, { a0 } from "./a";
import bd, * as b from "./b";
import cd, { c0 } from "./c";
import dd, { d0, d1 } from "./d";
import { Disposable } from "./disposable";
import e = require("./e");

ad; ad; a0; a0;
bd; bd; b.b0; b.b0;
cd; cd; c0; c0;
dd; dd; d0; d0; d1; d1;
e; e;
class X extends Disposable { }"#
                .to_string(),
        },
    );
    done();
}
