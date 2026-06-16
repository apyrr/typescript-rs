#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_add_missing_attributes10() {
    let mut t = TestingT;
    run_test_code_fix_add_missing_attributes10(&mut t);
}

fn run_test_code_fix_add_missing_attributes10(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @jsx: preserve
// @filename: foo.tsx
type A = 'a' | 'b' | 'c' | 'd' | 'e';
type B = 1 | 2 | 3;
type C = '@' | '!';
type D = ` + "`" + `${A}${Uppercase<A>}${B}${C}` + "`" + `;
const A = (props: { [K in D]: K }) =>
   <div {...props}></div>;

const Bar = () =>
   [|<A></A>|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &vec!["fixMissingAttributes".to_string()]);
    done();
}
