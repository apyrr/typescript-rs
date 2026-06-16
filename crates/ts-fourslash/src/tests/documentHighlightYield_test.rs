use crate::{new_fourslash, MarkerOrRangeOrName, TestingT};

pub fn test_document_highlight_yield(t: &mut TestingT) {
    let content = r#"
// @Filename: /a.ts
class C {
  async *[Symbol.asyncIterator]() {
    [|yield|] {
		type: 'type',
	};
  }
}
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let ranges = f.ranges();
    f.verify_baseline_document_highlights(
        t,
        None /*preferences*/,
        vec![MarkerOrRangeOrName::Range(ranges[0].clone())],
    );
    done();
}

