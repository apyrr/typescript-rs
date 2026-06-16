#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_incremental_update_to_class_implementing_generic_class() {
    let mut t = TestingT;
    run_test_incremental_update_to_class_implementing_generic_class(&mut t);
}

fn run_test_incremental_update_to_class_implementing_generic_class(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"declare function alert(message?: string): void;
class Animal<T> {
    constructor(public name: T) { }
    move(meters: number) {
        alert(this.name + " moved " + meters + "m.");
    }
}
class Animal2 extends Animal<string> {
    constructor(name: string) { super(name); }
    /*1*/get name2() { return this.name; }
}
var a = new Animal2('eprst');"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_no_errors();
    f.insert(t, "//");
    f.verify_no_errors();
    done();
}
