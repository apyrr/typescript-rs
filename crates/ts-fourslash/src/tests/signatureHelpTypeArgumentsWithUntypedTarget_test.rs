use crate::{new_fourslash, TestingT};

pub fn test_signature_help_on_type_arguments_with_unresolved_target(t: &mut TestingT) {
    let content = r#"
/*1*/un/*2*/resolvedVal/*3*/</*4*/Un/*5*/resolvedType/*6*/>/*7*/(/*8*/un/*9*/resolvedVal/*10*/);
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    for marker in f.marker_names() {
        f.go_to_marker(t, &marker);
        f.verify_no_signature_help(t);
    }
    done();
}

