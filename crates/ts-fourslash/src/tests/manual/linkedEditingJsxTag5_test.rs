use crate::{new_fourslash, skip_if_failing, TestingT};
use std::collections::BTreeMap;
use ts_lsproto as lsproto;

pub fn test_linked_editing_jsx_tag5(t: &mut TestingT) {
    skip_if_failing("TestLinkedEditingJsxTag5");
    let content = r#"// @FileName: /unclosedElement.tsx
const jsx = (
    <div/*0*/>
        </*1start*/div/*1*/>
    <//*2start*/div/*2*/>/*3*/
);/*4*/
// @FileName: /mismatchedElement.tsx
const jsx = (
    /*5*/</*6start*/div/*6*/>
        <//*7start*/div/*7*/>
    </*8*//div/*9*/>/*10*/
);
// @Filename: /invalidClosing.tsx
const jsx = (
   <di/*11*/v>
   </*12*/ //*13*/div>
);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let linked_cursors1 = vec![
        range_from_markers(&f, "1start", "1"),
        range_from_markers(&f, "2start", "2"),
    ];
    let linked_cursors2 = vec![
        range_from_markers(&f, "6start", "6"),
        range_from_markers(&f, "7start", "7"),
    ];

    f.verify_linked_editing_at_markers(
        t,
        BTreeMap::from([
            ("0".to_string(), Vec::new()),
            ("1".to_string(), linked_cursors1.clone()),
            ("2".to_string(), linked_cursors1),
            ("3".to_string(), Vec::new()),
            ("4".to_string(), Vec::new()),
            ("5".to_string(), Vec::new()),
            ("6".to_string(), linked_cursors2.clone()),
            ("7".to_string(), linked_cursors2),
            ("8".to_string(), Vec::new()),
            ("9".to_string(), Vec::new()),
            ("10".to_string(), Vec::new()),
            ("11".to_string(), Vec::new()), // this tag does not parse as a closing tag
            ("12".to_string(), Vec::new()),
            ("13".to_string(), Vec::new()),
        ]),
    );
    done();
}

fn range_from_markers(
    f: &crate::FourslashTest,
    start_marker: &str,
    end_marker: &str,
) -> lsproto::Range {
    lsproto::Range {
        start: f.marker_by_name(start_marker).ls_position,
        end: f.marker_by_name(end_marker).ls_position,
    }
}

