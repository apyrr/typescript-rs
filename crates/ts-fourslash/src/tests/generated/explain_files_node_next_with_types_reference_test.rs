#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_explain_files_node_next_with_types_reference() {
    let mut t = TestingT;
    run_test_explain_files_node_next_with_types_reference(&mut t);
}

fn run_test_explain_files_node_next_with_types_reference(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /node_modules/react-hook-form/package.json
{
  "name": "react-hook-form",
  "main": "dist/index.cjs.js",
  "module": "dist/index.esm.js",
  "types": "dist/index.d.ts",
  "exports": {
    "./package.json": "./package.json",
    ".": {
      "import": "./dist/index.esm.js",
      "require": "./dist/index.cjs.js",
      "types": "./dist/index.d.ts"
    }
  }
}
// @Filename: /node_modules/react-hook-form/dist/index.cjs.js
module.exports = {};
// @Filename: /node_modules/react-hook-form/dist/index.esm.js
export function useForm() {}
// @Filename: /node_modules/react-hook-form/dist/index.d.ts
/// <reference types="react/**/" />
export type Foo = React.Whatever;
export function useForm(): any;
// @Filename: /node_modules/react/index.d.ts
declare namespace JSX {}
declare namespace React { export interface Whatever {} }
// @Filename: /tsconfig.json
{
    "compilerOptions": {
        "module": "nodenext",
        "explainFiles": true
    }
    "files": ["./index.ts"]
}
// @Filename: /index.ts
import { useForm } from "react-hook-form";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
