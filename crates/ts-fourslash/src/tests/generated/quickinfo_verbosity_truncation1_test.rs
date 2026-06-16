#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_truncation1() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_truncation1(&mut t);
}

fn run_test_quickinfo_verbosity_truncation1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"type Str = string | {};
type FooType = Str | number;
type Sym = symbol | (() => void);
type BarType = Sym | boolean;
interface LotsOfProps {
    someLongPropertyName1: Str;
    someLongPropertyName2: FooType;
    someLongPropertyName3: Sym;
    someLongPropertyName4: BarType;
    someLongPropertyName5: Str;
    someLongPropertyName6: FooType;
    someLongPropertyName7: Sym;
    someLongPropertyName8: BarType;
    someLongMethodName1(a: FooType, b: BarType): Sym;
    someLongPropertyName9: Str;
    someLongPropertyName10: FooType;
    someLongPropertyName11: Sym;
    someLongPropertyName12: BarType;
    someLongPropertyName13: Str;
    someLongPropertyName14: FooType;
    someLongPropertyName15: Sym;
    someLongPropertyName16: BarType;
    someLongMethodName2(a: FooType, b: BarType): Sym;
}
const obj1/*o1*/: LotsOfProps = undefined as any as LotsOfProps;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([("o1".to_string(), vec![0, 1])]),
    );
    done();
}
