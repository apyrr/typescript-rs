use crate::{new_fourslash, TestingT};

pub fn test_quick_info_generic_property_accessor(t: &mut TestingT) {
    let content = r#"
declare const o: {
    f: <T>(x: T) => T
    get g(): <T>(x: T) => T
}

declare const x: number

o.f/*1*/(x)
o.g/*2*/(x)
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) f: <number>(x: number) => number", "");
    f.verify_quick_info_at(t, "2", "(accessor) g: <number>(x: number) => number", "");
    done();
}

