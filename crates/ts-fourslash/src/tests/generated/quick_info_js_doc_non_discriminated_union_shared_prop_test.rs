#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_non_discriminated_union_shared_prop() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_non_discriminated_union_shared_prop(&mut t);
}

fn run_test_quick_info_js_doc_non_discriminated_union_shared_prop(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDocNonDiscriminatedUnionSharedProp") {
        return;
    }
    let content = r#"// @strict: false
interface Entries {
  /**
   * Plugins info...
   */
  plugins?: Record<string, Record<string, unknown>>;
  /**
   * Output info...
   */
  output?: string;
  /**
   * Format info...
   */
  format?: string;
}

interface Input extends Entries {
  /**
   * Input info...
   */
  input: string;
}

interface Types extends Entries {
  /**
   * Types info...
   */
  types: string;
}

type EntriesOptions = Input | Types;

const options: EntriesOptions[] = [
  {
    input: "./src/index.ts",
    /*1*/output: "./dist/index.mjs",
  },
  {
    types: "./src/types.ts",
    format: "esm",
  },
];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "(property) Entries.output?: string",
        "Output info...",
    );
    done();
}
