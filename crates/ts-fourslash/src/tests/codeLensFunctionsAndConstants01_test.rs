use crate::{CodeLensUserPreferences, new_fourslash, TestingT, UserPreferences};

pub fn test_code_lens_functions_and_constants01(t: &mut TestingT) {
    let content = r#"
// @module: preserve

// @filename: ./exports.ts

let callCount = 0;
export function foo(n: number): void {
  callCount++;
  if (n > 0) {
	foo(n - 1);
  }
  else {
    console.log("function was called " + callCount + " times");
  }
}

foo(5);

export const bar = 123;

// @filename: ./importer.ts
import { foo, bar } from "./exports";

foo(5);
console.log(bar);
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_code_lens(
        t,
        Some(UserPreferences {
            code_lens: CodeLensUserPreferences {
                references_code_lens_enabled: Some(true),
                references_code_lens_show_on_all_functions: Some(true),
                implementations_code_lens_enabled: Some(true),
                implementations_code_lens_show_on_interface_methods: Some(true),
                implementations_code_lens_show_on_all_class_methods: Some(true),
            },
            ..UserPreferences::default()
        }),
    );
    done();
}

