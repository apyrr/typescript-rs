#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_references_binding_pattern_in_jsdoc_no_crash2() {
    let mut t = TestingT;
    run_test_find_references_binding_pattern_in_jsdoc_no_crash2(&mut t);
}

fn run_test_find_references_binding_pattern_in_jsdoc_no_crash2(t: &mut TestingT) {
    if should_skip_if_failing("TestFindReferencesBindingPatternInJsdocNoCrash2") {
        return;
    }
    let content = r#"// @moduleResolution: bundler
// @Filename: node_modules/use-query/package.json
{
  "name": "use-query",
  "types": "index.d.ts"
}
// @Filename: node_modules/use-query/index.d.ts
declare function useQuery(): {
  data: string[];
};
// @Filename: node_modules/use-query/package.json
{
  "name": "other",
  "types": "index.d.ts"
}
// @Filename: node_modules/other/index.d.ts
interface BottomSheetModalProps {
  /**
   * A scrollable node or normal view.
   * @type null | (({ data: any }?) => any)
   */
  children: null | (({ data: any }?) => any);
}
// @Filename: src/index.ts
import { useQuery } from "use-query";
const { /*1*/data } = useQuery();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
