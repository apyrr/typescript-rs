#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_declaration_keywords() {
    let mut t = TestingT;
    run_test_references_for_declaration_keywords(&mut t);
}

fn run_test_references_for_declaration_keywords(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Base {}
interface Implemented1 {}
/*classDecl1_classKeyword*/class C1 /*classDecl1_extendsKeyword*/extends Base /*classDecl1_implementsKeyword*/implements Implemented1 {
    /*getDecl_getKeyword*/get e() { return 1; }
    /*setDecl_setKeyword*/set e(v) {}
}
/*interfaceDecl1_interfaceKeyword*/interface I1 /*interfaceDecl1_extendsKeyword*/extends Base { }
/*typeDecl_typeKeyword*/type T = { }
/*enumDecl_enumKeyword*/enum E { }
/*namespaceDecl_namespaceKeyword*/namespace N { }
/*moduleDecl_moduleKeyword*/namespace M { }
/*functionDecl_functionKeyword*/function fn() {}
/*varDecl_varKeyword*/var x;
/*letDecl_letKeyword*/let y;
/*constDecl_constKeyword*/const z = 1;
interface Implemented2 {}
interface Implemented3 {}
class C2 /*classDecl2_implementsKeyword*/implements Implemented2, Implemented3 {}
interface I2 /*interfaceDecl2_extendsKeyword*/extends Implemented2, Implemented3 {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "classDecl1_classKeyword".to_string(),
            "classDecl1_extendsKeyword".to_string(),
            "classDecl1_implementsKeyword".to_string(),
            "classDecl2_implementsKeyword".to_string(),
            "getDecl_getKeyword".to_string(),
            "setDecl_setKeyword".to_string(),
            "interfaceDecl1_interfaceKeyword".to_string(),
            "interfaceDecl1_extendsKeyword".to_string(),
            "interfaceDecl2_extendsKeyword".to_string(),
            "typeDecl_typeKeyword".to_string(),
            "enumDecl_enumKeyword".to_string(),
            "namespaceDecl_namespaceKeyword".to_string(),
            "moduleDecl_moduleKeyword".to_string(),
            "functionDecl_functionKeyword".to_string(),
            "varDecl_varKeyword".to_string(),
            "letDecl_letKeyword".to_string(),
            "constDecl_constKeyword".to_string(),
        ],
    );
    done();
}
