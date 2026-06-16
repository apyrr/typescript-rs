#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_modifiers() {
    let mut t = TestingT;
    run_test_go_to_definition_modifiers(&mut t);
}

fn run_test_go_to_definition_modifiers(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: /a.ts
/*export*/export class A/*A*/ {

    /*private*/private z/*z*/: string;

    /*readonly*/readonly x/*x*/: string;

    /*async*/async a/*a*/() {  }

    /*override*/override b/*b*/() {}

    /*public1*/public/*public2*/ as/*multipleModifiers*/ync c/*c*/() { }
}

exp/*exportFunction*/ort function foo/*foo*/() { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "export".to_string(),
            "A".to_string(),
            "private".to_string(),
            "z".to_string(),
            "readonly".to_string(),
            "x".to_string(),
            "async".to_string(),
            "a".to_string(),
            "override".to_string(),
            "b".to_string(),
            "public1".to_string(),
            "public2".to_string(),
            "multipleModifiers".to_string(),
            "c".to_string(),
            "exportFunction".to_string(),
            "foo".to_string(),
        ],
    );
    done();
}
