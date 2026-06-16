#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_add_missing_attributes5() {
    let mut t = TestingT;
    run_test_code_fix_add_missing_attributes5(&mut t);
}

fn run_test_code_fix_add_missing_attributes5(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @jsx: preserve
// @filename: foo.tsx
interface P {
    a: number;
    b: string;
    c: number[];
    d: any;
}

const A = ({ a, b, c, d }: P) =>
    <div>{a}{b}{c}{d}</div>;

const Bar = () =>
    [|<A a={100} b={""} c={[]} d={undefined}></A>|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &vec!["fixMissingAttributes".to_string()]);
    done();
}
