#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_mapped_type_recursive_inference() {
    let mut t = TestingT;
    run_test_quick_info_mapped_type_recursive_inference(&mut t);
}

fn run_test_quick_info_mapped_type_recursive_inference(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: test.ts
interface A { a: A }
declare let a: A;
type Deep<T> = { [K in keyof T]: Deep<T[K]> }
declare function foo<T>(deep: Deep<T>): T;
const out/*1*/ = foo/*2*/(a);
out.a/*3*/
out.a.a/*4*/
out.a.a.a.a.a.a.a/*5*/

interface B { [s: string]: B }
declare let b: B;
const oub/*6*/ = foo/*7*/(b);
oub.b/*8*/
oub.b.b/*9*/
oub.b.a.n.a.n.a/*10*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "const out: {\n    a: {\n        a: ...;\n    };\n}",
        "",
    );
    f.verify_quick_info_at(t, "2", "function foo<{\n    a: {\n        a: ...;\n    };\n}>(deep: Deep<{\n    a: {\n        a: ...;\n    };\n}>): {\n    a: {\n        a: ...;\n    };\n}", "");
    f.verify_quick_info_at(
        t,
        "3",
        "(property) a: {\n    a: {\n        a: ...;\n    };\n}",
        "",
    );
    f.verify_quick_info_at(
        t,
        "4",
        "(property) a: {\n    a: {\n        a: ...;\n    };\n}",
        "",
    );
    f.verify_quick_info_at(
        t,
        "5",
        "(property) a: {\n    a: {\n        a: ...;\n    };\n}",
        "",
    );
    f.verify_quick_info_at(t, "6", "const oub: {\n    [x: string]: ...;\n}", "");
    f.verify_quick_info_at(t, "7", "function foo<{\n    [x: string]: ...;\n}>(deep: Deep<{\n    [x: string]: ...;\n}>): {\n    [x: string]: ...;\n}", "");
    f.verify_quick_info_at(t, "8", "{\n    [x: string]: ...;\n}", "");
    f.verify_quick_info_at(t, "9", "{\n    [x: string]: ...;\n}", "");
    f.verify_quick_info_at(t, "10", "{\n    [x: string]: ...;\n}", "");
    done();
}
