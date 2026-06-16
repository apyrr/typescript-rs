#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_from_jsx_tag() {
    let mut t = TestingT;
    run_test_completions_import_from_jsx_tag(&mut t);
}

fn run_test_completions_import_from_jsx_tag(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @jsx: react
// @Filename: /types.d.ts
declare namespace JSX {
  interface IntrinsicElements { a }
}
// @Filename: /Box.tsx
export function Box(props: any) { return null; }
// @Filename: /App.tsx
export function App() {
  return (
    <div className="App">
      <Box/**/
    </div>
  )
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_apply_code_action_from_completion(
        t,
        Some(""),
        &ApplyCodeActionFromCompletionOptions {
            name: "Box".to_string(),
            source: "./Box".to_string(),
            auto_import_fix: None,
            description: "Add import from \"./Box\"".to_string(),
            new_file_content: Some(
                r#"import { Box } from "./Box";

export function App() {
  return (
    <div className="App">
      <Box
    </div>
  )
}"#
                .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    done();
}
