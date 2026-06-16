#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_auto_imports_re_exports() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_auto_imports_re_exports(&mut t);
}

fn run_test_code_fix_class_implement_interface_auto_imports_re_exports(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: node_modules/test-module/index.d.ts
declare namespace e {
    interface Foo {}
}
export = e;
// @Filename: a.ts
import { Foo } from "test-module";
export interface A {
    foo(): Foo;
}
// @Filename: b.ts
import { A } from "./a";
export class B implements A {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "b.ts");
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'A'".to_string(),
            new_file_content: r#"import { Foo } from "test-module";
import { A } from "./a";
export class B implements A {
    foo(): Foo {
        throw new Error("Method not implemented.");
    }
}"#
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
