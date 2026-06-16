use crate::{new_fourslash, skip_if_failing, TestingT};
use std::collections::BTreeMap;
use ts_lsproto as lsproto;

pub fn test_linked_editing_jsx_tag6(t: &mut TestingT) {
    skip_if_failing("TestLinkedEditingJsxTag6");
    let content = r#"// @Filename: /namespace.tsx
const jsx = (
    </*start*/someNamespa/*3*/ce./*2*/Thing/*startend*/>
    <//*end*/someNamespace/*1*/.Thing/*endend*/>
);
 const jsx1 = </*4*/foo/*5*/  /*6*/./*7*/ /*8*/ba/*9*/r><//*10*/foo.bar>;
 const jsx2 = <foo./*11*/bar><//*12*/ /*13*/f/*14*/oo /*15*/./*16*/b/*17*/ar/*18*/>;
 const jsx3 = </*19*/foo/*20*/ //*21*// /*22*/some comment
     /*23*/./*24*/bar>
     </f/*25*/oo.bar>;
 let jsx4 =
     </*26*/foo  /*27*/ .// hi/*28*/
     /*29*/bar/*26end*/>
     <//*30*/foo  /*31*/ .// hi/*32*/
     /*33*/bar/*30end*/>"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    let linked_cursors1 = vec![
        range_from_markers(&f, "start", "startend"),
        range_from_markers(&f, "end", "endend"),
    ];
    let linked_cursors2 = vec![
        range_from_markers(&f, "26", "26end"),
        range_from_markers(&f, "30", "30end"),
    ];
    f.verify_linked_editing_at_markers(
        t,
        BTreeMap::from([
            ("1".to_string(), linked_cursors1.clone()),
            ("2".to_string(), linked_cursors1.clone()),
            ("3".to_string(), linked_cursors1),
            ("4".to_string(), Vec::new()),
            ("5".to_string(), Vec::new()),
            ("6".to_string(), Vec::new()),
            ("7".to_string(), Vec::new()),
            ("8".to_string(), Vec::new()),
            ("9".to_string(), Vec::new()),
            ("10".to_string(), Vec::new()),
            ("11".to_string(), Vec::new()),
            ("12".to_string(), Vec::new()),
            ("13".to_string(), Vec::new()),
            ("14".to_string(), Vec::new()),
            ("15".to_string(), Vec::new()),
            ("16".to_string(), Vec::new()),
            ("17".to_string(), Vec::new()),
            ("18".to_string(), Vec::new()),
            ("19".to_string(), Vec::new()),
            ("20".to_string(), Vec::new()),
            ("21".to_string(), Vec::new()),
            ("22".to_string(), Vec::new()),
            ("23".to_string(), Vec::new()),
            ("24".to_string(), Vec::new()),
            ("25".to_string(), Vec::new()),
            ("26".to_string(), linked_cursors2.clone()),
            ("27".to_string(), linked_cursors2.clone()),
            ("28".to_string(), linked_cursors2.clone()),
            ("29".to_string(), linked_cursors2.clone()),
            ("30".to_string(), linked_cursors2.clone()),
            ("31".to_string(), linked_cursors2.clone()),
            ("32".to_string(), linked_cursors2.clone()),
            ("33".to_string(), linked_cursors2),
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

