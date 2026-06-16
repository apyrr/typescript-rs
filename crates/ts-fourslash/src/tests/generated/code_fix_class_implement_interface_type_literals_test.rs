#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_type_literals() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_type_literals(&mut t);
}

fn run_test_code_fix_class_implement_interface_type_literals(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassImplementInterfaceTypeLiterals") {
        return;
    }
    let content = r"type Builtin = Date | Function | Uint8Array | string | number | boolean | undefined;

export type DeepPartial<T> = T extends Builtin ? T :
    T extends Array<infer U> ? Array<DeepPartial<U>> :
        T extends ReadonlyArray<infer U> ? ReadonlyArray<DeepPartial<U>> :
            T extends {} ? { [K in keyof T]?: DeepPartial<T[K]> } : Partial<T>;

export interface Nested {
    field: string;
}

interface Foo {
    request(): DeepPartial<{ nested1: Nested; test2: Nested }>;
}
[|export class C implements Foo {}|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'Foo'".to_string(),
            new_file_content: String::new(),
            new_range_content: r#"export class C implements Foo {
    request(): DeepPartial<{ nested1: Nested; test2: Nested; }> {
        throw new Error("Method not implemented.");
    }
}"#
            .to_string(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
