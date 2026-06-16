#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_class2() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_class2(&mut t);
}

fn run_test_quickinfo_verbosity_class2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface Apple {
    color: string;
}
class Foo/*1*/<T> {
    constructor(public x: T) { }
    public y!: T;
    static whatever(): void { }
    private foo(): Apple { return { color: "green" }; }
    static {
        const a = class { x?: Apple; };
    }
    protected z = true;
}
type Whatever/*2*/ = Foo<string>;
const a/*3*/ = Foo;
const c/*4*/ = Foo<string>;
[1].forEach(class/*5*/ <T> {
    constructor(public x: T) { }
    public y!: T;
    static whatever(): void { }
    private foo(): Apple { return { color: "green" }; }
    static {
        const a = class { x?: Apple; };
    }
    protected z = true;
});
const b/*6*/ = Bar<number>;
@random()
abstract class Animal/*7*/ {
    name!: string;
    abstract makeSound(): void;
}
class Dog/*8*/ {
    what(this: this, that: Dog) { }
    #bones: string[];
}
const d/*9*/ = new Dog();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("1".to_string(), vec![0, 1, 2]),
            ("2".to_string(), vec![0, 1, 2]),
            ("3".to_string(), vec![0, 1]),
            ("4".to_string(), vec![0]),
            ("5".to_string(), vec![0, 1, 2]),
            ("6".to_string(), vec![0]),
            ("7".to_string(), vec![0, 1]),
            ("8".to_string(), vec![0, 1]),
            ("9".to_string(), vec![0, 1]),
        ]),
    );
    done();
}
