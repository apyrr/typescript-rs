#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_combinators3() {
    let mut t = TestingT;
    run_test_generic_combinators3(&mut t);
}

fn run_test_generic_combinators3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface Collection<T, U> {
}

interface Combinators {
    map<T, U, V>(c: Collection<T,U>, f: (x: T, y: U) => V): Collection<T, V>;
    map<T, U>(c: Collection<T,U>, f: (x: T, y: U) => any): Collection<any, any>;
}

var c2: Collection<number, string>;

var _: Combinators;

var /*9*/r1a  = _.ma/*1c*/p(c2, (/*1a*/x,/*1b*/y) => { return x + "" });  // check quick info of map here"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1a", "(parameter) x: number", "");
    f.verify_quick_info_at(t, "1b", "(parameter) y: string", "");
    f.verify_quick_info_at(t, "1c", "(method) Combinators.map<number, string, string>(c: Collection<number, string>, f: (x: number, y: string) => string): Collection<number, string> (+1 overload)", "");
    f.verify_quick_info_at(t, "9", "var r1a: Collection<number, string>", "");
    done();
}
