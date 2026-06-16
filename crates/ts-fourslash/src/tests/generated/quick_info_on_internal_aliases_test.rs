#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_internal_aliases() {
    let mut t = TestingT;
    run_test_quick_info_on_internal_aliases(&mut t);
}

fn run_test_quick_info_on_internal_aliases(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/** Module comment*/
export namespace m1 {
    /** m2 comments*/
    export namespace m2 {
        /** class comment;*/
        export class /*1*/c {
        };
    }
    export function foo() {
    }
}
/**This is on import declaration*/
import /*2*/internalAlias = m1.m2./*3*/c;
var /*4*/newVar = new /*5*/internalAlias();
var /*6*/anotherAliasVar = /*7*/internalAlias;
import /*8*/internalFoo = m1./*9*/foo;
var /*10*/callVar = /*11*/internalFoo();
var /*12*/anotherAliasFoo = /*13*/internalFoo;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "class m1.m2.c", "class comment;");
    f.verify_quick_info_at(
        t,
        "2",
        "(alias) class internalAlias\nimport internalAlias = m1.m2.c",
        "This is on import declaration",
    );
    f.verify_quick_info_at(t, "3", "class m1.m2.c", "class comment;");
    f.verify_quick_info_at(t, "4", "var newVar: internalAlias", "");
    f.verify_quick_info_at(
        t,
        "5",
        "(alias) new internalAlias(): internalAlias\nimport internalAlias = m1.m2.c",
        "This is on import declaration",
    );
    f.verify_quick_info_at(t, "6", "var anotherAliasVar: typeof internalAlias", "");
    f.verify_quick_info_at(
        t,
        "7",
        "(alias) class internalAlias\nimport internalAlias = m1.m2.c",
        "This is on import declaration",
    );
    f.verify_quick_info_at(
        t,
        "8",
        "(alias) function internalFoo(): void\nimport internalFoo = m1.foo",
        "",
    );
    f.verify_quick_info_at(t, "9", "function m1.foo(): void", "");
    f.verify_quick_info_at(t, "10", "var callVar: void", "");
    f.verify_quick_info_at(
        t,
        "11",
        "(alias) internalFoo(): void\nimport internalFoo = m1.foo",
        "",
    );
    f.verify_quick_info_at(t, "12", "var anotherAliasFoo: () => void", "");
    f.verify_quick_info_at(
        t,
        "13",
        "(alias) function internalFoo(): void\nimport internalFoo = m1.foo",
        "",
    );
    done();
}
