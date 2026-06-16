use crate::{new_fourslash, skip_if_failing, TestingT};
use std::collections::BTreeMap;
use ts_lsproto as lsproto;

pub fn test_linked_editing_jsx_tag1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /basic.tsx
/*a*/const j/*b*/sx = (
    /*c*/</*0*/d/*1*/iv/*2*/>/*3*/
    </*4*///*5*/di/*6*/v/*7*/>/*8*/
);
const jsx2 = (
    </*9start*/d/*9*/iv/*9end*/>
        </*10start*/d/*10*/iv/*10end*/>
            </*11start*/p/*11*/>
            <//*12*/p/*12end*/>        
        <//*13start*/d/*13*/iv/*13end*/>
    <//*14start*/d/*14*/iv/*14end*/>
);/*d*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let linked_cursors1 = vec![
        range_from_markers(&f, "0", "2"),
        range_from_markers(&f, "5", "7"),
    ];
    let linked_cursors2 = vec![
        range_from_markers(&f, "9start", "9end"),
        range_from_markers(&f, "14start", "14end"),
    ];
    let linked_cursors3 = vec![
        range_from_markers(&f, "10start", "10end"),
        range_from_markers(&f, "13start", "13end"),
    ];
    let linked_cursors4 = vec![
        range_from_markers(&f, "11start", "11"),
        range_from_markers(&f, "12", "12end"),
    ];
    f.verify_linked_editing_at_markers(
        t,
        BTreeMap::from([
            ("0".to_string(), linked_cursors1.clone()),
            ("1".to_string(), linked_cursors1.clone()),
            ("2".to_string(), linked_cursors1.clone()),
            ("3".to_string(), Vec::new()),
            ("4".to_string(), Vec::new()),
            ("5".to_string(), linked_cursors1.clone()),
            ("6".to_string(), linked_cursors1.clone()),
            ("7".to_string(), linked_cursors1),
            ("8".to_string(), Vec::new()),
            ("9".to_string(), linked_cursors2.clone()),
            ("10".to_string(), linked_cursors3.clone()),
            ("11".to_string(), linked_cursors4.clone()),
            ("12".to_string(), linked_cursors4),
            ("13".to_string(), linked_cursors3),
            ("14".to_string(), linked_cursors2),
            ("a".to_string(), Vec::new()),
            ("b".to_string(), Vec::new()),
            ("c".to_string(), Vec::new()),
            ("d".to_string(), Vec::new()),
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

