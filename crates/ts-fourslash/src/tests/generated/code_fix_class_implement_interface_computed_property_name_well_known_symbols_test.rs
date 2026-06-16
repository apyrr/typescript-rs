#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_computed_property_name_well_known_symbols() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_computed_property_name_well_known_symbols(&mut t);
}

fn run_test_code_fix_class_implement_interface_computed_property_name_well_known_symbols(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r#"// @strict: false
// @lib: es2017
interface I<Species> {
    [Symbol.hasInstance](o: any): boolean;
    [Symbol.isConcatSpreadable]: boolean;
    [Symbol.iterator](): any;
    [Symbol.match]: boolean;
    [Symbol.replace](...args);
    [Symbol.search](str: string): number;
    [Symbol.species](): Species;
    [Symbol.split](str: string, limit?: number): string[];
    [Symbol.toPrimitive](hint: "number"): number;
    [Symbol.toPrimitive](hint: "default"): number;
    [Symbol.toPrimitive](hint: "string"): string;
    [Symbol.toStringTag]: string;
    [Symbol.unscopables]: any;
}
class C implements I<number> {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'I<number>'".to_string(),
            new_file_content: r#"interface I<Species> {
    [Symbol.hasInstance](o: any): boolean;
    [Symbol.isConcatSpreadable]: boolean;
    [Symbol.iterator](): any;
    [Symbol.match]: boolean;
    [Symbol.replace](...args);
    [Symbol.search](str: string): number;
    [Symbol.species](): Species;
    [Symbol.split](str: string, limit?: number): string[];
    [Symbol.toPrimitive](hint: "number"): number;
    [Symbol.toPrimitive](hint: "default"): number;
    [Symbol.toPrimitive](hint: "string"): string;
    [Symbol.toStringTag]: string;
    [Symbol.unscopables]: any;
}
class C implements I<number> {
    [Symbol.hasInstance](o: any): boolean {
        throw new Error("Method not implemented.");
    }
    [Symbol.isConcatSpreadable]: boolean;
    [Symbol.iterator]() {
        throw new Error("Method not implemented.");
    }
    [Symbol.match]: boolean;
    [Symbol.replace](...args: any[]) {
        throw new Error("Method not implemented.");
    }
    [Symbol.search](str: string): number {
        throw new Error("Method not implemented.");
    }
    [Symbol.species](): number {
        throw new Error("Method not implemented.");
    }
    [Symbol.split](str: string, limit?: number): string[] {
        throw new Error("Method not implemented.");
    }
    [Symbol.toPrimitive](hint: "number"): number;
    [Symbol.toPrimitive](hint: "default"): number;
    [Symbol.toPrimitive](hint: "string"): string;
    [Symbol.toPrimitive](hint: unknown): string | number {
        throw new Error("Method not implemented.");
    }
    [Symbol.toStringTag]: string;
    [Symbol.unscopables]: any;
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
