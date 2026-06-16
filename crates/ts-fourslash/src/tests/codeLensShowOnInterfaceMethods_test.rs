use crate::{CodeLensUserPreferences, new_fourslash, TestingT, UserPreferences};
use ts_core::Tristate;

pub fn test_code_lens_references_show_on_interface_methods(t: &mut TestingT) {
    let containing_test_name = "TestCodeLensReferencesShowOnInterfaceMethods";
    for value in [Tristate::True, Tristate::False] {
        let _subtest_name = format!("{}={}", containing_test_name, value.is_true());
        let content = r#"
export interface I {
  methodA(): void;
}
export interface I {
  methodB(): void;
}

interface J extends I {
  methodB(): void;
  methodC(): void;
}

class C implements J {
  methodA(): void {}
  methodB(): void {}
  methodC(): void {}
}

class AbstractC implements J {
  abstract methodA(): void;
  methodB(): void {}
  abstract methodC(): void;
}
"#;
        let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
        f.verify_baseline_code_lens(
            t,
            Some(UserPreferences {
                code_lens: CodeLensUserPreferences {
                    implementations_code_lens_enabled: Some(true),
                    implementations_code_lens_show_on_interface_methods: Some(value.is_true()),
                    ..CodeLensUserPreferences::default()
                },
                ..UserPreferences::default()
            }),
        );
        done();
    }
}

