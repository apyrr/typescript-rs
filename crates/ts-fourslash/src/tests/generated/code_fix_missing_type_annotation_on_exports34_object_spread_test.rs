#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports34_object_spread() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports34_object_spread(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports34_object_spread(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports34-object-spread") {
        return;
    }
    let content = r#"// @isolatedDeclarations: true
// @declaration: true
// @Filename: /code.ts
const Start = {
  A: 'A',
  B: 'B',
} as const;

const End = {
  Y: "Y",
  Z: "Z"
} as const;
export const All_Part1 = {};
function getPart() {
  return { M: "Z"}
}

export const All = {
  x: 1,
  ...Start,
  y: 1,
  ...getPart(),
  ...End,
  z: 1,
};"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(t, VerifyCodeFixOptions {
    description: "Add annotation of type 'typeof All_Part1_1 & typeof Start & typeof All_Part3 & typeof All_Part4 & typeof End & typeof All_Part6'".to_string(),
    new_file_content: r#"const Start = {
  A: 'A',
  B: 'B',
} as const;

const End = {
  Y: "Y",
  Z: "Z"
} as const;
export const All_Part1 = {};
function getPart() {
  return { M: "Z"}
}

const All_Part1_1 = {
    x: 1
};
const All_Part3 = {
    y: 1
};
const All_Part4 = getPart();
const All_Part6 = {
    z: 1
};
export const All: typeof All_Part1_1 & typeof Start & typeof All_Part3 & typeof All_Part4 & typeof End & typeof All_Part6 = {
    ...All_Part1_1,
    ...Start,
    ...All_Part3,
    ...All_Part4,
    ...End,
    ...All_Part6
};"#.to_string(),
    new_range_content: String::new(),
    index: 1,
    apply_changes: false,
    user_preferences: None,
});
    done();
}
