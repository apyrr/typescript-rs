#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_remove_semicolons3() {
    let mut t = TestingT;
    run_test_format_remove_semicolons3(&mut t);
}

fn run_test_format_remove_semicolons3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"(<InterfaceTypeWithDeclaredMembers>type).declaredProperties = getNamedMembers(members);
// Start with signatures at empty array in case of recursive types
(<InterfaceTypeWithDeclaredMembers>type).declaredCallSignatures = emptyArray;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings.semicolons = lsutil::SemicolonPreference::Remove;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"(<InterfaceTypeWithDeclaredMembers>type).declaredProperties = getNamedMembers(members);
// Start with signatures at empty array in case of recursive types
(<InterfaceTypeWithDeclaredMembers>type).declaredCallSignatures = emptyArray",
    );
    done();
}
