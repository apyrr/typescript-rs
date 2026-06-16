use crate::{CodeLensUserPreferences, new_fourslash, TestingT, UserPreferences};

pub fn test_code_lens_overloads01(t: &mut TestingT) {
    let content = r#"
export function foo(x: number): number;
export function foo(x: string): string;
export function foo(x: string | number): string | number {
	return x;
}

foo(1);

foo("hello");

// This one isn't expected to match any overload,
// but is really just here to test how it affects how code lens.
foo(Math.random() ? 1 : "hello");
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

