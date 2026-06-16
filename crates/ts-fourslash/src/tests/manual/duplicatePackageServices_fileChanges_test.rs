use crate::{new_fourslash, TestingT};

pub fn test_duplicate_package_services_file_changes(t: &mut TestingT) {
    let content = r#"// @noImplicitReferences: true
// @Filename: /node_modules/a/index.d.ts
import X from "x";
export function a(x: X): void;
// @Filename: /node_modules/a/node_modules/x/index.d.ts
export default class /*defAX*/X {
    private x: number;
}
// @Filename: /node_modules/a/node_modules/x/package.json
{ "name": "x", "version": "1.2./*aVersionPatch*/3" }
// @Filename: /node_modules/b/index.d.ts
import X from "x";
export const b: X;
// @Filename: /node_modules/b/node_modules/x/index.d.ts
export default class /*defBX*/X {
    private x: number;
}
// @Filename: /node_modules/b/node_modules/x/package.json
{ "name": "x", "version": "1.2./*bVersionPatch*/3" }
// @Filename: /src/a.ts
import { a } from "a";
import { b } from "b";
a(/*error*/b);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    f.go_to_file(t, "/src/a.ts");
    f.verify_number_of_errors_in_current_file(0);

    test_change_and_change_back(&mut f, t, "aVersionPatch", "defAX");
    test_change_and_change_back(&mut f, t, "bVersionPatch", "defBX");
    done();
}

fn test_change_and_change_back(
    f: &mut crate::FourslashTest,
    t: &mut TestingT,
    version_patch: &str,
    def: &str,
) {
    // Insert "4" after the version patch marker, changing version from 1.2.3 to 1.2.43
    f.go_to_marker(t, version_patch);
    f.insert(t, "4");

    // Insert a space after the definition marker to trigger a recheck
    f.go_to_marker(t, def);
    f.insert(t, " ");

    // No longer have identical packageId, so we get errors.
    f.verify_error_exists_after_marker_name("error");

    // Undo the changes
    f.go_to_marker(t, version_patch);
    f.delete_at_caret(t, 1);
    f.go_to_marker(t, def);
    f.delete_at_caret(t, 1);

    // Back to being identical.
    f.go_to_file(t, "/src/a.ts");
    f.verify_number_of_errors_in_current_file(0);
}

