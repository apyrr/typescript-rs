#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_class1() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_class1(&mut t);
}

fn run_test_quickinfo_verbosity_class1(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickinfoVerbosityClass1") {
        return;
    }
    let content = r#"{
    class Foo {
        a!: "a" | "c";
    }
    const f/*f1*/ = new Foo();
}
{
    type FooParam = "a" | "b";
    class Foo {
        constructor(public x: string) {
            this.x = "a";
        }
        foo(p: FooParam): void {}
    }
    const f/*f2*/ = new Foo("");
}
{
    class Bar/*B*/ {
        a!: string;
        bar(): void {}
        baz(param: string): void {}
    }
    class Foo extends Bar {
        b!: boolean;
        override baz(param: string | number): void {}
    }
    const f/*f3*/ = new Foo();
}
{
    class Bar<B extends string> {
        bar(param: B): void {}
        baz(): this { return this; }
    }
    class Foo extends Bar<"foo"> {
        foo(): this { return this; }
    }
    const b/*b1*/ = new Bar();
    const f/*f4*/ = new Foo();
}
{
    class Bar<B extends string> {
        bar(param: B): void {}
        baz(): this { return this; }
    }
    const noname/*n1*/ = new (class extends Bar<"foo"> {
        foo(): this { return this; }
    })();
    const klass = class extends Bar<"foo"> {
        foo(): this { return this; }
    };
    const k/*k1*/ = new klass();
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("f1".to_string(), vec![0, 1]),
            ("f2".to_string(), vec![0, 1, 2]),
            ("f3".to_string(), vec![0, 1]),
            ("b1".to_string(), vec![0, 1]),
            ("f4".to_string(), vec![0, 1]),
            ("n1".to_string(), vec![0, 1]),
            ("k1".to_string(), vec![0, 1]),
            ("B".to_string(), vec![0, 1]),
        ]),
    );
    done();
}
