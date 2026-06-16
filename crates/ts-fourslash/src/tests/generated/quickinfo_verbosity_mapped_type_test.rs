#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_mapped_type() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_mapped_type(&mut t);
}

fn run_test_quickinfo_verbosity_mapped_type(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"type Apple = boolean | number;
type Orange = string | boolean;
type F<T> = {
	[K in keyof T as T[K] extends Apple ? never : K]: T[K];
}
type Bar = {
	banana: string;
	apple: boolean;
}
const x/*x*/: F/*F*/<Bar> = { banana: 'hello' };
const y/*y*/: { [K in keyof Bar]?: Bar[K] } = { banana: 'hello' };
type G<T> = {
	[K in keyof T]: T[K] & Apple
};
const z: G/*G*/<Bar> = { banana: 'hello', apple: true };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("x".to_string(), vec![0, 1]),
            ("y".to_string(), vec![0]),
            ("F".to_string(), vec![0, 1]),
            ("G".to_string(), vec![0, 1]),
        ]),
    );
    done();
}
