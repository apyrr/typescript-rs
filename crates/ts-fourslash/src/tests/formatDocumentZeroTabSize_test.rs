use crate::{new_fourslash, TestingT};
use ts_core::Tristate;

pub fn test_format_document_zero_tab_size(t: &mut TestingT) {
    let content = r#"function foo() {
    if (true) {
        var x = 1;
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let mut opts = f.get_options();
    opts.format_code_settings.editor_settings.tab_size = 0;
    opts.format_code_settings.editor_settings.indent_size = 0;
    opts.format_code_settings.editor_settings.convert_tabs_to_spaces = Tristate::True;
    f.configure(t, opts);
    f.format_document(t, "");
    f.verify_current_file_content(t, "function foo() {\nif (true) {\nvar x = 1;\n}\n}");
    done();
}

