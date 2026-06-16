#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_no_destructure_non_object_literal() {
    let mut t = TestingT;
    run_test_import_name_code_fix_no_destructure_non_object_literal(&mut t);
}

fn run_test_import_name_code_fix_no_destructure_non_object_literal(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_noDestructureNonObjectLiteral") {
        return;
    }
    let content = r"// @lib: es5
// @target: es2015
// @strict: true
// @esModuleInterop: true
// @Filename: /array.ts
declare const arr: number[];
export = arr;
// @Filename: /class-instance-member.ts
class C { filter() {} }
export = new C();
// @Filename: /object-literal.ts
declare function filter(): void;
export = { filter };
// @Filename: /jquery.d.ts
interface JQueryStatic {
  filter(): void;
}
declare const $: JQueryStatic;
export = $;
// @Filename: /jquery.js
module.exports = {};
// @Filename: /index.ts
filter/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(
        t,
        "",
        &vec!["./object-literal".to_string(), "./jquery".to_string()],
        None,
    );
    done();
}
