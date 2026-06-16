use crate::{new_fourslash, TestingT};

// Test that auto-imports for JSX tags don't crash when React is type-imported.
// When both the JSX namespace (React) and the component need to be imported,
// getSymbolNamesToImport returns multiple names and the type-only promotion
// path should handle this gracefully instead of panicking.
pub fn test_code_fix_promote_type_only_import_jsx_tag(t: &mut TestingT) {
    let content = r#"// @module: preserve
// @verbatimModuleSyntax: true
// @jsx: react
// @Filename: /react.ts
const React: any = {};
export default React;
// @Filename: /bar.tsx
import type React from "./react";

<Foo/**/ />;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    // The fix should promote the type-only import of React to a regular import.
    // The "Cannot find name 'Foo'" error does not produce an auto-import for
    // React since it's already imported (as type-only, handled by promotion).
    let expected = vec![r#"import React from "./react";

<Foo />;"#
        .to_string()];
    f.verify_import_fix_at_position(t, &expected, None /*preferences*/);
    done();
}

// Test edge case where both the component name (Foo) and the JSX namespace (React)
// are type-only imported. Each diagnostic is matched to its symbol via the error
// message, so each produces only its own promotion fix (no duplicates).
pub fn test_code_fix_promote_type_only_import_jsx_tag_both_type_only(t: &mut TestingT) {
    let content = r#"// @module: preserve
// @verbatimModuleSyntax: true
// @jsx: react
// @Filename: /react.ts
const React: any = {};
export default React;
// @Filename: /foo.ts
export function Foo() { return null; }
// @Filename: /bar.tsx
import type React from "./react";
import type { Foo } from "./foo";

<Foo/**/ />;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    // Both Foo and React are type-only imported. The error message string
    // matching disambiguates which diagnostic is about which symbol, so each
    // diagnostic produces only its own promotion fix (no duplicates).
    let expected = vec![
        r#"import type React from "./react";
import { Foo } from "./foo";

<Foo />;"#
            .to_string(),
        r#"import React from "./react";
import type { Foo } from "./foo";

<Foo />;"#
            .to_string(),
    ];
    f.verify_import_fix_at_position(t, &expected, None /*preferences*/);
    done();
}
