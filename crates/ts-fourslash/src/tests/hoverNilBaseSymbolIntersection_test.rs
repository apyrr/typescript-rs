use crate::{new_fourslash, TestingT};

pub fn test_hover_nil_base_symbol_intersection(t: &mut TestingT) {
    let content = r#"
// @strict: true
// @filename: main.ts

class Base {}

declare const BaseFactory: new() => Base & { c: string };

class Derived extends BaseFactory {
  static /*1*/idField = "id" as const;
}
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    // We only care that hover/quickinfo does not crash (panic) when baseType.Symbol() is nil.
    // Pre-fix (#2763), hovering on the static property could panic.
    f.verify_baseline_hover(t, &[]);
    done();
}
