#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_recursive_type() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_recursive_type(&mut t);
}

fn run_test_quickinfo_verbosity_recursive_type(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @lib: es5
type Node/*N*/<T> = {
    value: T;
    left: Node<T> | undefined;
    right: Node<T> | undefined;
}
const n/*n*/: Node<number> = {
    value: 1,
    left: undefined,
    right: undefined,
}
interface Orange {
    name: string;
}
type TreeNode/*t*/<T> = {
    value: T;
    left: TreeNode<T> | undefined;
    right: TreeNode<T> | undefined;
    orange?: Orange;
}
const m/*m*/: TreeNode<number> = {
    value: 1,
    left: undefined,
    right: undefined,
    orange: { name: "orange" },
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("N".to_string(), vec![0]),
            ("n".to_string(), vec![0, 1]),
            ("t".to_string(), vec![0, 1]),
            ("m".to_string(), vec![0, 1, 2]),
        ]),
    );
    done();
}
