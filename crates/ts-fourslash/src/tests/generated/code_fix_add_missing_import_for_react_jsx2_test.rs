#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_add_missing_import_for_react_jsx2() {
    let mut t = TestingT;
    run_test_code_fix_add_missing_import_for_react_jsx2(&mut t);
}

fn run_test_code_fix_add_missing_import_for_react_jsx2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @jsx: react-jsxdev
// @Filename: node_modules/react/index.d.ts
export declare var React: any;
// @Filename: node_modules/react/package.json
{
  "name": "react",
  "types": "./index.d.ts"
}
// @Filename: foo.tsx
 export default function Foo(){
     return <></>;
 }
// @Filename: bar.tsx
 export default function Bar(){
     return <Foo></Foo>;
 }
// @Filename: package.json
{
  "dependencies": {
    "react": "*"
  }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "bar.tsx");
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingImport".to_string(),
            new_file_content: r#"import Foo from "./foo";

export default function Bar(){
    return <Foo></Foo>;
}"#
            .to_string(),
        },
    );
    done();
}
