#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_duplicate_class_module_error0() {
    let mut t = TestingT;
    run_test_duplicate_class_module_error0(&mut t);
}

fn run_test_duplicate_class_module_error0(t: &mut TestingT) {
    if should_skip_if_failing("TestDuplicateClassModuleError0") {
        return;
    }
    let content = r#"module A
{
    class B
    {
        public Hello(): string
        {
            return "from private B";
        }
    }
}

module A
{
 /*1*/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.disable_formatting();
    f.go_to_marker(t, "1");
    f.insert(t, "    export class B\n    {\n        public Hello(): string\n        {\n            return \"from export B\";\n        }\n    }\n");
    f.insert(t, "\n");
    done();
}
