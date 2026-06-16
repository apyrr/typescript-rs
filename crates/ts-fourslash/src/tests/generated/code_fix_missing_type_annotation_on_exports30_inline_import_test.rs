#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports30_inline_import() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports30_inline_import(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports30_inline_import(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @isolatedDeclarations: true
// @declaration: true
// @Filename: /person-code.ts
export type Person = { x: string; }
export function getPerson() : Person {
  return null!
}
// @Filename: /code.ts
import { getPerson } from "./person-code";
export const exp = {
  person: getPerson()
};"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/code.ts");
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add satisfies and an inline type assertion with 'Person'".to_string(),
            new_file_content: r#"import { getPerson, Person } from "./person-code";
export const exp = {
  person: getPerson() satisfies Person as Person
};"#
            .to_string(),
            new_range_content: String::new(),
            index: 1,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
