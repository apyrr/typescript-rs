use crate::{new_fourslash, TestingT};

pub fn test_folding_range_jsx_property_access(t: &mut TestingT) {
    let content = r#"// @jsx: preserve
// @Filename: /a.tsx
const Components =[| {
  Nested: () => null
}|];

export const Test = () =>[| {
  return [|<Components.Nested></Components.Nested>|];
}|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}

