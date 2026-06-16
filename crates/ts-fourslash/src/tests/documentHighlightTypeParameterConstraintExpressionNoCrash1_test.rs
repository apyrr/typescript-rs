use crate::{new_fourslash, MarkerOrRangeOrName, TestingT};

pub fn test_document_highlight_type_parameter_constraint_expression_no_crash1(t: &mut TestingT) {
    let content = r#"// @Filename: /a.ts
const v/*m*/alue = 1;
type Box<T extends +value> = typeof value"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None /*preferences*/,
        vec![MarkerOrRangeOrName::Name("m".to_string())],
    );
    done();
}

