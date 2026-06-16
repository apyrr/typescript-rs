#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_nested_generic_calls() {
    let mut t = TestingT;
    run_test_quick_info_nested_generic_calls(&mut t);
}

fn run_test_quick_info_nested_generic_calls(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @strict: true
/*1*/m({ foo: /*2*/$("foo") });
m({ foo: /*3*/$("foo") });
declare const m: <S extends string>(s: { [_ in S]: { $: NoInfer<S> } }) => void
declare const $: <S, T extends S>(s: T) => { $: S }
type NoInfer<T> = [T][T extends any ? 0 : never];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "const m: <\"foo\">(s: {\n    foo: {\n        $: \"foo\";\n    };\n}) => void",
        "",
    );
    f.verify_quick_info_at(
        t,
        "2",
        "const $: <unknown, string>(s: string) => {\n    $: unknown;\n}",
        "",
    );
    f.verify_quick_info_at(
        t,
        "3",
        "const $: <unknown, string>(s: string) => {\n    $: unknown;\n}",
        "",
    );
    done();
}
