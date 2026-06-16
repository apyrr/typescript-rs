use crate::{new_fourslash, skip_if_failing, TestingT};
use std::collections::BTreeMap;
use ts_lsproto as lsproto;

pub fn test_linked_editing_jsx_tag7(t: &mut TestingT) {
    skip_if_failing("TestLinkedEditingJsxTag7");
    let content = r#"// @FileName: /fragment.tsx
/*a*/const j/*b*/sx =/*c*/ (
    /*5*/</*0*/>/*1*/
        <img />
    /*6*/</*2*///*3*/>/*4*/
)/*d*/;
const jsx2 = (
    /* this is comment *//*13*/</*10*//* /*11*/more comment *//*12*/>/*8*/Hello/*9*/
    <//*14*/ /*18*///*17*/* even/*15*/ more comment *//*16*/>
);
const jsx3 = (
    <>/*7*/
    </>
);/*e*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let start_range = f.marker_by_name("0").ls_position;
    let end_range = f.marker_by_name("3").ls_position;
    let linked_cursors1 = vec![
        lsproto::Range {
            start: start_range,
            end: start_range,
        },
        lsproto::Range {
            start: end_range,
            end: end_range,
        },
    ];
    let start_range2 = f.marker_by_name("10").ls_position;
    let end_range2 = f.marker_by_name("14").ls_position;
    let linked_cursors2 = vec![
        lsproto::Range {
            start: start_range2,
            end: start_range2,
        },
        lsproto::Range {
            start: end_range2,
            end: end_range2,
        },
    ];
    f.verify_linked_editing_at_markers(
        t,
        BTreeMap::from([
            ("0".to_string(), linked_cursors1.clone()),
            ("1".to_string(), Vec::new()),
            ("2".to_string(), Vec::new()),
            ("3".to_string(), linked_cursors1),
            ("4".to_string(), Vec::new()),
            ("5".to_string(), Vec::new()),
            ("6".to_string(), Vec::new()),
            ("7".to_string(), Vec::new()),
            ("8".to_string(), Vec::new()),
            ("9".to_string(), Vec::new()),
            ("10".to_string(), linked_cursors2.clone()),
            ("11".to_string(), Vec::new()),
            ("12".to_string(), Vec::new()),
            ("13".to_string(), Vec::new()),
            ("14".to_string(), linked_cursors2),
            ("15".to_string(), Vec::new()),
            ("16".to_string(), Vec::new()),
            ("17".to_string(), Vec::new()),
            ("18".to_string(), Vec::new()),
            ("a".to_string(), Vec::new()),
            ("b".to_string(), Vec::new()),
            ("c".to_string(), Vec::new()),
            ("d".to_string(), Vec::new()),
            ("e".to_string(), Vec::new()),
        ]),
    );
    done();
}

