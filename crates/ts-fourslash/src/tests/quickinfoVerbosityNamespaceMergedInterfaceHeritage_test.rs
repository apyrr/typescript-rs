use crate::{new_fourslash, TestingT};
use std::collections::BTreeMap;

pub fn test_quickinfo_verbosity_namespace_merged_interface_heritage(t: &mut TestingT) {
    let content = r#"
declare namespace NS/*1*/ {
    interface Config extends A {
        a: string;
    }

    interface Config extends B {
        b: number;
    }

    interface A {
        a: string;
    }

    interface B {
        b: number;
    }
}
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        BTreeMap::from([("1".to_string(), vec![0, 1])]),
    );
    done();
}

