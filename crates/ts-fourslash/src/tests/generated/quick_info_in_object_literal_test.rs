#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_in_object_literal() {
    let mut t = TestingT;
    run_test_quick_info_in_object_literal(&mut t);
}

fn run_test_quick_info_in_object_literal(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoInObjectLiteral") {
        return;
    }
    let content = r#"interface Foo {
    doStuff(x: string, callback: (a: string) => string);
}
var x1: Foo = {
    y/*1*/1: () => {
        return "";
    } ,
    doStuff: (z, callback) => { return callback(this.y); }
}
var value = 3;
class Foo {
    static getRandomPosition() {
        return {
            "row": v/*2*/alue
        }
  }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) y1: () => string", "");
    f.verify_quick_info_at(t, "2", "var value: number", "");
    done();
}
