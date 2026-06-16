use crate::{new_fourslash, TestingT};

pub fn test_format_on_enter_in_comment(t: &mut TestingT) {
    let content = r#"   /**
    * /*1*/
    */"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert_line(t, "");
    f.verify_current_file_content(
        t,
        r#"  /**
   * 

   */"#,
    );
    done();
}

