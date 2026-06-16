#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_bloom_filters2() {
    let mut t = TestingT;
    run_test_references_bloom_filters2(&mut t);
}

fn run_test_references_bloom_filters2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: declaration.ts
var container = { /*1*/42: 1 };
// @Filename: expression.ts
function blah() { return (container[42]) === 2;  };
// @Filename: stringIndexer.ts
function blah2() { container["42"] };
// @Filename: redeclaration.ts
container = { "42" : 18 };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
