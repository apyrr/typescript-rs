use crate::{new_fourslash, skip_if_failing, TestingT};
use std::collections::BTreeMap;
use ts_lsproto as lsproto;

pub fn test_linked_editing_jsx_tag4(t: &mut TestingT) {
    skip_if_failing("TestLinkedEditingJsxTag4");
    let content = r#"// @Filename: /typeTag.tsx
const jsx = (
   </*0*/div/*1*/</*2*/T/*3*/>/*4*/>/*5*/
      <p>
         <img />
      </p>
   <//*6*/div/*7*/>
);
// @Filename: /typeTagError.tsx
const jsx = (
   </*10*/div/*11*/</*12*/T/*13*/>/*14*/
      </*15*/p />
   <//*16*/div>
);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let linked_cursors = vec![
        range_from_markers(&f, "0", "1"),
        range_from_markers(&f, "6", "7"),
    ];
    f.verify_linked_editing_at_markers(
        t,
        BTreeMap::from([
            ("0".to_string(), linked_cursors.clone()),
            ("1".to_string(), linked_cursors.clone()),
            ("2".to_string(), Vec::new()),
            ("3".to_string(), Vec::new()),
            ("4".to_string(), Vec::new()),
            ("5".to_string(), Vec::new()),
            ("6".to_string(), linked_cursors),
            ("10".to_string(), Vec::new()),
            ("11".to_string(), Vec::new()),
            ("12".to_string(), Vec::new()),
            ("13".to_string(), Vec::new()),
            ("14".to_string(), Vec::new()),
            ("15".to_string(), Vec::new()),
            ("16".to_string(), Vec::new()),
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

