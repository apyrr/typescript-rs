#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_auto_imports_type_only() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_auto_imports_type_only(&mut t);
}

fn run_test_code_fix_class_implement_interface_auto_imports_type_only(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassImplementInterfaceAutoImports_typeOnly") {
        return;
    }
    let content = r"// @module: esnext
// @verbatimModuleSyntax: true
// @Filename: types1.ts
type A = {};
export default A;
// @Filename: types2.ts
export type B = {};
export type C = {};
export type D<T> = {};
// @Filename: interface.ts
import type A from './types1';
import type { B, C, D } from './types2';

export interface Base {
  a: A;
  b<T extends B = B>(p1: C): D<C>;
}
// @Filename: index.ts
import type { Base } from './interface';

export class C implements Base {[| |]}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "index.ts");
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'Base'".to_string(),
            new_file_content: r"import type { Base } from './interface';
import type A from './types1';
import type { B, C, D } from './types2';

export class C implements Base {
    a: A;
    b<T extends B = B>(p1: C): D<C> {
        throw new Error('Method not implemented.');
    }
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
