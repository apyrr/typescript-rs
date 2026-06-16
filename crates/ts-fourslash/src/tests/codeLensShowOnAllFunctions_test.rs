use crate::{CodeLensUserPreferences, new_fourslash, TestingT, UserPreferences};
use ts_core::Tristate;

pub fn test_code_lens_references_show_on_all_functions(t: &mut TestingT) {
    let containing_test_name = "TestCodeLensReferencesShowOnAllFunctions";
    for value in [Tristate::True, Tristate::False] {
        let _subtest_name = format!("{}={}", containing_test_name, value.is_true());
        let content = r#"
export function f1(): void {}

function f2(): void {}

export const f3 = () => {};

const f4 = () => {};

const f5 = function() {};
"#;
        let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
        f.verify_baseline_code_lens(
            t,
            Some(UserPreferences {
                code_lens: CodeLensUserPreferences {
                    references_code_lens_enabled: Some(true),
                    references_code_lens_show_on_all_functions: Some(value.is_true()),
                    ..CodeLensUserPreferences::default()
                },
                ..UserPreferences::default()
            }),
        );
        done();
    }
}

