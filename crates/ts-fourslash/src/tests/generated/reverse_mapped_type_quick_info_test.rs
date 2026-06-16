#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_reverse_mapped_type_quick_info() {
    let mut t = TestingT;
    run_test_reverse_mapped_type_quick_info(&mut t);
}

fn run_test_reverse_mapped_type_quick_info(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface IAction {
    type: string;
}

type Reducer<S> = (state: S, action: IAction) => S

function combineReducers<S>(reducers: { [K in keyof S]: Reducer<S[K]> }): Reducer<S> {
    const dummy = {} as S;
    return () => dummy;
}

const test_inner = (test: string, action: IAction) => {
    return 'dummy';
}
const test = combineReducers({
    test_inner
});

const test_outer = combineReducers({
    test
});

// '{test: { test_inner: any } }'
type FinalType/*1*/ = ReturnType<typeof test_outer>;

var k: FinalType;
k.test.test_inner/*2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "type FinalType = {\n    test: {\n        test_inner: string;\n    };\n}",
        "",
    );
    f.verify_quick_info_at(t, "2", "(property) test_inner: string", "");
    done();
}
