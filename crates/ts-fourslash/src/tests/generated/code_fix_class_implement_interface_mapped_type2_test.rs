#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_mapped_type2() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_mapped_type2(&mut t);
}

fn run_test_code_fix_class_implement_interface_mapped_type2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"type ListenerTemplate<T, S extends string, I extends string = "${1}"> = {
    [K in keyof T as K extends string
        ? S extends ` + "`" + `${infer F}${I}${infer R}` + "`" + ` ? ` + "`" + `${F}${K}${R}` + "`" + ` : K : K]
        : (listener: (payload: T[K]) => void) => void;
};
type ListenActionable<E> = ListenerTemplate<E, "add*Listener" | "remove*Listener", "*">;
type ClickEventSupport = ListenActionable<{ Click: 'some-click-event-payload' }>;

[|class C implements ClickEventSupport { }|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'ClickEventSupport'".to_string(),
            new_file_content: String::new(),
            new_range_content: r#"class C implements ClickEventSupport {
    addClickListener: (listener: (payload: "some-click-event-payload") => void) => void;
    removeClickListener: (listener: (payload: "some-click-event-payload") => void) => void;
}"#
            .to_string(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
