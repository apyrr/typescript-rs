use crate::{new_fourslash, skip_if_failing, TestingT};
use std::collections::BTreeMap;
use ts_lsproto as lsproto;

pub fn test_linked_editing_jsx_tag9(t: &mut TestingT) {
    skip_if_failing("TestLinkedEditingJsxTag9");
    let content = r#"// @Filename: /whitespace.tsx
const whitespaceOpening = (
   </*0*/ /*1*/div/*2*/ /*3*/> /*4*/
   <//*5*/di/*6*/v/*5end*/>
);
const whitespaceClosing = (
   </*7*/di/*8*/v/*8end*/>
   <//*9*/ /*10*/div/*11*/ /*12*/> /*13*/
);
const triviaOpening = (
    /* this is/*14*/ comment *//*15*/</*16*//* /*17*/more/*18*/ comment *//*19*/ /*20start*/di/*20*/v/*20end*/ /* comments */>/*21*/Hello/*22*/
    <//*23*/ /*24*///*25*/* even/*26*/ more comment *//*27*/ /*28start*/d/*28*/iv/*28end*/ /* b/*29*/ye */>
);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    let linked_cursors1 = vec![
        range_from_markers(&f, "1", "2"),
        range_from_markers(&f, "5", "5end"),
    ];
    let linked_cursors2 = vec![
        range_from_markers(&f, "7", "8end"),
        range_from_markers(&f, "10", "11"),
    ];
    let linked_cursors3 = vec![
        range_from_markers(&f, "20start", "20end"),
        range_from_markers(&f, "28start", "28end"),
    ];

    f.verify_linked_editing_at_markers(
        t,
        BTreeMap::from([
            ("0".to_string(), Vec::new()),
            ("1".to_string(), linked_cursors1.clone()),
            ("2".to_string(), linked_cursors1.clone()),
            ("3".to_string(), Vec::new()),
            ("4".to_string(), Vec::new()),
            ("5".to_string(), linked_cursors1.clone()),
            ("6".to_string(), linked_cursors1),
            ("7".to_string(), linked_cursors2.clone()),
            ("8".to_string(), linked_cursors2.clone()),
            ("9".to_string(), Vec::new()),
            ("10".to_string(), linked_cursors2.clone()),
            ("11".to_string(), linked_cursors2),
            ("12".to_string(), Vec::new()),
            ("13".to_string(), Vec::new()),
            ("14".to_string(), Vec::new()),
            ("15".to_string(), Vec::new()),
            ("16".to_string(), Vec::new()),
            ("17".to_string(), Vec::new()),
            ("18".to_string(), Vec::new()),
            ("19".to_string(), Vec::new()),
            ("20".to_string(), linked_cursors3.clone()),
            ("21".to_string(), Vec::new()),
            ("22".to_string(), Vec::new()),
            ("23".to_string(), Vec::new()),
            ("24".to_string(), Vec::new()),
            ("25".to_string(), Vec::new()),
            ("26".to_string(), Vec::new()),
            ("27".to_string(), Vec::new()),
            ("28".to_string(), linked_cursors3),
            ("29".to_string(), Vec::new()),
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

