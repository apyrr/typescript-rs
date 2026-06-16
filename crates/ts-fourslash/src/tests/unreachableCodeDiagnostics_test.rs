use crate::{new_fourslash, TestingT};

pub fn test_unreachable_code_diagnostics(t: &mut TestingT) {
    let content = r#"// @allowUnreachableCode: false
throw new Error();
	
(() => {})();
	"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_non_suggestion_diagnostics(t);
    done();
}

