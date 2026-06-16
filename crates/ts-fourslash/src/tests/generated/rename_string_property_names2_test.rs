#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_string_property_names2() {
    let mut t = TestingT;
    run_test_rename_string_property_names2(&mut t);
}

fn run_test_rename_string_property_names2(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameStringPropertyNames2") {
        return;
    }
    let content = r#"type Props = {
  foo: boolean;
}

let { foo }: Props = null as any;
foo;

let asd: Props = { "foo"/**/: true }; // rename foo here"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &["".to_string()]);
    done();
}
