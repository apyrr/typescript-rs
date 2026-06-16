use crate::{CodeLensUserPreferences, new_fourslash, TestingT, UserPreferences};

pub fn test_code_lens_function_expressions01(t: &mut TestingT) {
    let content = r#"
// @filename: anonymousFunctionExpressions.ts
export let anonFn1 = function () {};
export const anonFn2 = function () {};

let anonFn3 = function () {};
const anonFn4 = function () {};

// @filename: arrowFunctions.ts
export let arrowFn1 = () => {};
export const arrowFn2 = () => {};

let arrowFn3 = () => {};
const arrowFn4 = () => {};

// @filename: namedFunctions.ts
export let namedFn1 = function namedFn1() {
    namedFn1();
}
namedFn1();

export const namedFn2 = function namedFn2() {
    namedFn2();
}
namedFn2();

let namedFn3 = function namedFn3() {};
const namedFn4 = function namedFn4() {};
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

