use crate::{new_fourslash, skip_if_failing, TestingT};

// Test case for crash when promoting type-only import to value import
// when existing type imports precede the new value import
// https://github.com/microsoft/typescript-go/issues/2559
pub fn test_code_fix_promote_type_only_ordering_crash(t: &mut TestingT) {
    skip_if_failing("TestCodeFixPromoteTypeOnlyOrderingCrash");
    let content = r#"// @module: node18
// @verbatimModuleSyntax: true
// @Filename: /bar.ts
export interface AAA {}
export class BBB {}
// @Filename: /foo.ts
import type {
    AAA,
    BBB,
} from "./bar";

let x: AAA = new BBB()"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/foo.ts");

    let expected = vec![r#"import {
    BBB,
    type AAA,
} from "./bar";

let x: AAA = new BBB()"#
        .to_string()];
    f.verify_import_fix_at_position(t, &expected, None /*preferences*/);
    done();
}

