#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_interface2() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_interface2(&mut t);
}

fn run_test_quickinfo_verbosity_interface2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"{
    interface Foo/*1*/ {
        a: "a" | "c";
    }
}
{
    interface Bar {
        b: "b" | "d";
    }
    interface Foo/*2*/ extends Bar {
        a: "a" | "c";
    }
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
    interface Foo/*3*/ extends Bar {
        a: FooType;
        foo: (a: FooParam) => number;
    }
}
{
    interface Bar/*4*/<B> {
        bar(b: B): string;
    }
    interface FooParam {
        param: "a" | "c";
    }
    interface Foo/*5*/ extends Bar<FooParam> {
        a: "a" | "c";
        foo: (a: FooParam) => number;
    }
}
{
    interface Foo {
        a: "a";
    }
    interface Foo/*6*/ {
        b: "b";
    }
}
interface Foo/*7*/ {
    a: "a";
}
namespace Foo/*8*/ {
    export const bar: string;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("1".to_string(), vec![0, 1]),
            ("2".to_string(), vec![0, 1]),
            ("3".to_string(), vec![0, 1, 2]),
            ("4".to_string(), vec![0, 1]),
            ("5".to_string(), vec![0, 1, 2]),
            ("6".to_string(), vec![0, 1]),
            ("7".to_string(), vec![0, 1]),
            ("8".to_string(), vec![0, 1]),
        ]),
    );
    done();
}
