#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_type_definition_aliases() {
    let mut t = TestingT;
    run_test_go_to_type_definition_aliases(&mut t);
}

fn run_test_go_to_type_definition_aliases(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: goToTypeDefinitioAliases_module1.ts
interface /*definition*/I {
    p;
}
export {I as I2};
// @Filename: goToTypeDefinitioAliases_module2.ts
import {I2 as I3} from "./goToTypeDefinitioAliases_module1";
var v1: I3;
export {v1 as v2};
// @Filename: goToTypeDefinitioAliases_module3.ts
import {/*reference1*/v2 as v3} from "./goToTypeDefinitioAliases_module2";
/*reference2*/v3;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_type_definition(
        t,
        &["reference1".to_string(), "reference2".to_string()],
    );
    done();
}
