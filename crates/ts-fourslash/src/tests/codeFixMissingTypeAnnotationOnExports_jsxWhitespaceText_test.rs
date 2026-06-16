use crate::{new_fourslash, TestingT};

pub fn test_code_fix_missing_type_annotation_on_exports_jsx_whitespace_text(t: &mut TestingT) {
    let content = r#"// @isolatedDeclarations: true
// @declaration: true
// @module: preserve
// @Filename: /test.tsx
export const /**/elem = <div>
    <span />
</div>;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    f.go_to_marker(t, "");
    f.verify_code_fix_available(t, None);
    done();
}

