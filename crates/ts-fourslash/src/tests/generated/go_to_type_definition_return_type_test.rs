#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_type_definition_return_type() {
    let mut t = TestingT;
    run_test_go_to_type_definition_return_type(&mut t);
}

fn run_test_go_to_type_definition_return_type(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface /*I*/I { x: number; }
interface /*J*/J { y: number; }

function f0(): I { return { x: 0 }; }

type T = /*T*/(i: I) => I;
const f1: T = i => ({ x: i.x + 1 });

const f2 = (i: I): I => ({ x: i.x + 1 });

const f3 = (i: I) => (/*f3Def*/{ x: i.x + 1 });

const f4 = (i: I) => i;

const f5 = /*f5Def*/(i: I): I | J => ({ x: i.x + 1 });

const f6 = (i: I, j: J, b: boolean) => b ? i : j;

const /*f7Def*/f7 = (i: I) => {};

function f8(i: I): I;
function f8(j: J): J;
function /*f8Def*/f8(ij: any): any { return ij; }

/*f0*/f0();
/*f1*/f1();
/*f2*/f2();
/*f3*/f3();
/*f4*/f4();
/*f5*/f5();
/*f6*/f6();
/*f7*/f7();
/*f8*/f8();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_type_definition(
        t,
        &[
            "f0".to_string(),
            "f1".to_string(),
            "f2".to_string(),
            "f3".to_string(),
            "f4".to_string(),
            "f5".to_string(),
            "f6".to_string(),
            "f7".to_string(),
            "f8".to_string(),
        ],
    );
    done();
}
