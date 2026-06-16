#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_index_signature() {
    let mut t = TestingT;
    run_test_go_to_definition_index_signature(&mut t);
}

fn run_test_go_to_definition_index_signature(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionIndexSignature") {
        return;
    }
    let content = r"interface I {
    /*defI*/[x: string]: boolean;
}
interface J {
    /*defJ*/[x: string]: number;
}
interface K {
    /*defa*/[x: `a${string}`]: string;
    /*defb*/[x: `${string}b`]: string;
}
declare const i: I;
i.[|/*useI*/foo|];
declare const ij: I | J;
ij.[|/*useIJ*/foo|];
declare const k: K;
k.[|/*usea*/a|];
k.[|/*useb*/b|];
k.[|/*useab*/ab|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "useI".to_string(),
            "useIJ".to_string(),
            "usea".to_string(),
            "useb".to_string(),
            "useab".to_string(),
        ],
    );
    done();
}
