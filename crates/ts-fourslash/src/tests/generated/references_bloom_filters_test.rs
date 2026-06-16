#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_bloom_filters() {
    let mut t = TestingT;
    run_test_references_bloom_filters(&mut t);
}

fn run_test_references_bloom_filters(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: declaration.ts
var container = { /*1*/searchProp : 1 };
// @Filename: expression.ts
function blah() { return (1 + 2 + container.searchProp()) === 2;  };
// @Filename: stringIndexer.ts
function blah2() { container["searchProp"] };
// @Filename: redeclaration.ts
container = { "searchProp" : 18 };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
