#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickfix_implement_interface_unreachable_type_uses_relative_import() {
    let mut t = TestingT;
    run_test_quickfix_implement_interface_unreachable_type_uses_relative_import(&mut t);
}

fn run_test_quickfix_implement_interface_unreachable_type_uses_relative_import(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: class.ts
export class Class { }
// @Filename: interface.ts
import { Class } from './class';

export interface Foo {
    x: Class;
}
// @Filename: index.ts
import { Foo } from './interface';

class /*1*/X implements Foo {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'Foo'".to_string(),
            new_file_content: String::new(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
