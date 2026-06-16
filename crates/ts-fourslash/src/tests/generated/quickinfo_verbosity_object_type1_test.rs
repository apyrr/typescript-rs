#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_object_type1() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_object_type1(&mut t);
}

fn run_test_quickinfo_verbosity_object_type1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"type Str = string | {};
type FooType = Str | number;
type Sym = symbol | (() => void);
type BarType = Sym | boolean;
type Obj = { foo: FooType, bar: BarType, str: Str };
const obj1/*o1*/: Obj = { foo: 1, bar: true, str: "3"};
const obj2/*o2*/: { foo: FooType, bar: BarType, str: Str } = { foo: 1, bar: true, str: "3"};"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("o1".to_string(), vec![0, 1, 2, 3]),
            ("o2".to_string(), vec![0, 1, 2]),
        ]),
    );
    done();
}
