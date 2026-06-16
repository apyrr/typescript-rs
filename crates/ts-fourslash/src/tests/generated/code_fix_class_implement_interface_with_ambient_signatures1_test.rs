#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_with_ambient_signatures1() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_with_ambient_signatures1(&mut t);
}

fn run_test_code_fix_class_implement_interface_with_ambient_signatures1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @lib: esnext
// @target: esnext
// @Filename: /node_modules/@types/node/globals.d.ts
export {};
declare global {
    interface SymbolConstructor {
        readonly dispose: unique symbol;
    }
    interface Disposable {
        [Symbol.dispose](): void;
    }
}
// @Filename: /node_modules/@types/node/index.d.ts
/// <reference path="globals.d.ts" />
// @Filename: a.ts
class Foo implements Disposable {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "a.ts");
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'Disposable'".to_string(),
            new_file_content: r#"class Foo implements Disposable {
    [Symbol.dispose](): void {
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
