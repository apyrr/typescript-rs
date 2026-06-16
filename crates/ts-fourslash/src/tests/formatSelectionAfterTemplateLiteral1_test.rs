use crate::{new_fourslash, TestingT};

pub fn test_format_selection_after_template_literal1(t: &mut TestingT) {
    let content = "const a = `head${\"x\"};\n`;\n\n/*begin*/export const f = () => {\n    return `world`;\n/*end*/}\n";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_selection(t, "begin", "end");
    f.verify_current_file_content(
        t,
        "const a = `head${\"x\"};\n`;\n\nexport const f = () => {\n    return `world`;\n}\n",
    );
    done();
}

