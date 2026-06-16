use std::collections::BTreeMap;

use crate::{new_fourslash, TestingT};

pub fn test_auto_close_tag(t: &mut TestingT) {
    // Using separate files for each example to avoid unclosed JSX tags affecting other tests.
    let content = r#"// @noLib: true

// @Filename: /0.tsx
const x = <div>/*0*/;

// @Filename: /1.tsx
const x = <div> foo/*1*/ </div>;

// @Filename: /2.tsx
const x = <div></div>/*2*/;

// @Filename: /3.tsx
const x = <div/>/*3*/;

// @Filename: /4.tsx
const x = <div>
    <p>/*4*/
    </div>
</p>;

// @Filename: /5.tsx
const x = <div> text /*5*/;

// @Filename: /6.tsx
const x = <div>
    <div>/*6*/
</div>;

// @Filename: /7.tsx
const x = <div>
    <p>/*7*/
</div>;

// @Filename: /8.tsx
const x = <div>
    <div>/*8*/</div>
</div>;

// @Filename: /9.tsx
const x = <p>
    <div>
        <div>/*9*/
    </div>
</p>"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_jsx_closing_tags(
        t,
        BTreeMap::from([
            ("0".to_string(), Some("</div>".to_string())),
            ("1".to_string(), None),
            ("2".to_string(), None),
            ("3".to_string(), None),
            ("4".to_string(), Some("</p>".to_string())),
            ("5".to_string(), Some("</div>".to_string())),
            ("6".to_string(), Some("</div>".to_string())),
            ("7".to_string(), Some("</p>".to_string())),
            ("8".to_string(), None),
            ("9".to_string(), Some("</div>".to_string())),
        ]),
    );
    done();
}

