use crate::{new_fourslash, InlayHintsPreferences, TestingT, UserPreferences};
use ts_core::Tristate;

pub fn test_inlay_hints_using(t: &mut TestingT) {
    let content = r#"// @target: esnext
using _defer = {
	[Symbol.dispose]() {},
};"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints_with_preferences(
        t,
        None,
        &UserPreferences {
            inlay_hints: InlayHintsPreferences {
                include_inlay_variable_type_hints: Tristate::True,
                ..InlayHintsPreferences::default()
            },
            ..UserPreferences::default()
        },
    );
    done();
}

