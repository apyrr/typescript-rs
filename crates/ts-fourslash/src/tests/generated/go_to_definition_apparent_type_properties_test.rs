#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_apparent_type_properties() {
    let mut t = TestingT;
    run_test_go_to_definition_apparent_type_properties(&mut t);
}

fn run_test_go_to_definition_apparent_type_properties(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface Number {
    /*definition*/myObjectMethod(): number;
}

var o = 0;
o.[|/*reference1*/myObjectMethod|]();
o[[|"/*reference2*/myObjectMethod"|]]();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["reference1".to_string(), "reference2".to_string()]);
    done();
}
