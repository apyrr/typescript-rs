#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_global() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_global(&mut t);
}

fn run_test_code_fix_class_implement_interface_global(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassImplementInterfaceGlobal") {
        return;
    }
    let content = r"// @Filename: /src/globals.d.ts
export {}; // Make this a module
declare global {
    interface Disposable {
        [Symbol.dispose](): void;
    }
}
// @Filename: /src/test.ts
import { Service } from './lifecycle';
export class [|EditingService|] implements Service { }
// @Filename: /src/lifecycle.ts
export interface Disposable {
	(): string;
}
export interface Service {
	d: Disposable;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/src/test.ts");
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'Service'".to_string(),
            new_file_content: r"import { Disposable, Service } from './lifecycle';
export class EditingService implements Service {
    d: Disposable;
}"
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
