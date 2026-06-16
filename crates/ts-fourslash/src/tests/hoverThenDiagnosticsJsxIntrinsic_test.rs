use crate::{new_fourslash, TestingT};

// Regression test for https://github.com/microsoft/typescript-go/issues/3638
//
// A `textDocument/hover` on a JSX intrinsic element used to cause the
// subsequent `textDocument/diagnostic` pull to spuriously report
// TS2304: Cannot find name 'div'.
pub fn test_hover_then_diagnostics_jsx_intrinsic(t: &mut TestingT) {
    let content = r#"// @Filename: /tsconfig.json
{ "compilerOptions": { "strict": true, "jsx": "preserve" } }
// @Filename: /jsx.d.ts
declare namespace JSX {
    interface Element { }
    interface IntrinsicElements {
        div: any;
    }
}
// @Filename: /file.tsx
export default function Home() {
    return <di/*1*/v>hi</div>;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    // Hover on the intrinsic element first...
    f.verify_quick_info_at(t, "1", "(property) JSX.IntrinsicElements.div: any", "");
    // ...then a subsequent diagnostic pull must not invent a TS2304 for `div`.
    f.verify_no_errors();
    done();
}

