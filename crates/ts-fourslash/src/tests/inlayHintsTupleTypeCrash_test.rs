use crate::{new_fourslash, InlayHintsPreferences, TestingT, UserPreferences};
use ts_core::Tristate;

pub fn test_inlay_hints_tuple_type_crash(t: &mut TestingT) {
    let content = r#"function iterateTuples(tuples: [string][]): void {
    tuples.forEach((l) => {})
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints_with_preferences(
        t,
        None,
        &UserPreferences {
            inlay_hints: InlayHintsPreferences {
                include_inlay_function_parameter_type_hints: Tristate::True,
                ..InlayHintsPreferences::default()
            },
            ..UserPreferences::default()
        },
    );
    done();
}

