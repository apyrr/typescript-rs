use crate::{CodeLensUserPreferences, new_fourslash, TestingT, UserPreferences};

pub fn test_code_lens_interface01(t: &mut TestingT) {
    let content = r#"
// @module: preserve

// @filename: ./pointable.ts
export interface Pointable {
  getX(): number;
  getY(): number;
}

// @filename: ./classPointable.ts
import { Pointable } from "./pointable";

class Point implements Pointable {
  getX(): number {
    return 0;
  }
  getY(): number {
    return 0;
  }
}

// @filename: ./objectPointable.ts
import { Pointable } from "./pointable";

let x = 0;
let y = 0;
const p: Pointable = {
  getX(): number {
	return x;
  },
  getY(): number {
	return y;
  },
};
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

