use crate::{new_fourslash, TestingT};

pub fn test_quick_info_for_overload_on_const1(t: &mut TestingT) {
    let content = r#"interface I {
    x/*1*/1(a: number, callback: (x: 'hi') => number);
}
class C {
    x/*2*/1(a: number, call/*3*/back: (x: 'hi') => number);
    x/*4*/1(a: number, call/*5*/back: (x: string) => number) {
        call/*6*/back('hi');
        callback('bye');
        var hm = "hm";
        callback(hm);
    }
}
var c: C;
c.x/*7*/1(1, (x/*8*/x: 'hi') => { return 1; } );
c.x1(1, (x/*9*/x: 'bye') => { return 1; } );
c.x1(1, (x/*10*/x) => { return 1; } );"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "(method) I.x1(a: number, callback: (x: 'hi') => number): any",
        "",
    );
    f.verify_quick_info_at(
        t,
        "2",
        "(method) C.x1(a: number, callback: (x: 'hi') => number): any",
        "",
    );
    f.verify_quick_info_at(t, "3", "(parameter) callback: (x: 'hi') => number", "");
    f.verify_quick_info_at(
        t,
        "4",
        "(method) C.x1(a: number, callback: (x: string) => number): void",
        "",
    );
    f.verify_quick_info_at(t, "5", "(parameter) callback: (x: string) => number", "");
    f.verify_quick_info_at(t, "6", "(parameter) callback: (x: string) => number", "");
    f.verify_quick_info_at(
        t,
        "7",
        "(method) C.x1(a: number, callback: (x: 'hi') => number): any",
        "",
    );
    f.verify_quick_info_at(t, "8", "(parameter) xx: \"hi\"", "");
    f.verify_quick_info_at(t, "9", "(parameter) xx: \"bye\"", "");
    f.verify_quick_info_at(t, "10", "(parameter) xx: \"hi\"", "");
    done();
}

