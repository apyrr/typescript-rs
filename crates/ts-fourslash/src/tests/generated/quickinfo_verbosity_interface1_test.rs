#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_interface1() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_interface1(&mut t);
}

fn run_test_quickinfo_verbosity_interface1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"{
    interface Foo {
        a: "a" | "c";
    }
    const f/*f1*/: Foo = { a: "a" };
}
{
    interface Bar {
        b: "b" | "d";
    }
    interface Foo extends Bar {
        a: "a" | "c";
    }
    const f/*f2*/: Foo = { a: "a", b: "b" };
}
{
    type BarParam = "b" | "d";
    interface Bar {
        bar(b: BarParam): string;
    }
    type FooType = "a" | "c";
    interface FooParam {
        param: FooType;
    }
    interface Foo extends Bar {
        a: FooType;
        foo: (a: FooParam) => number;
    }
    const f/*f3*/: Foo = { a: "a", bar: () => "b", foo: () => 1 };
}
{
    interface Bar<B> {
        bar(b: B): string;
    }
    interface FooParam {
        param: "a" | "c";
    }
    interface Foo extends Bar<FooParam> {
        a: "a" | "c";
        foo: (a: FooParam) => number;
    }
    const f/*f4*/: Foo = { a: "a", bar: () => "b", foo: () => 1 };
    const b/*b1*/: Bar<number> = { bar: () => "" };
}
{
    interface Foo<A> {
        a: A;
    }
    type Alias = Foo<string>;
    const a/*a*/: Alias = { a: "a" };
}
{
    interface Foo {
        a: "a";
    }
    interface Foo {
        b: "b";
    }
    const f/*f5*/: Foo = { a: "a", b: "b" };
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("f1".to_string(), vec![0, 1]),
            ("f2".to_string(), vec![0, 1]),
            ("f3".to_string(), vec![0, 1, 2, 3]),
            ("f4".to_string(), vec![0, 1, 2]),
            ("b1".to_string(), vec![0, 1]),
            ("a".to_string(), vec![0, 1, 2]),
            ("f5".to_string(), vec![0, 1]),
        ]),
    );
    done();
}
