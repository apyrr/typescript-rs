use std::path::PathBuf;

use crate::filefixture::{Fixture, from_file, from_string};

pub fn bench_fixtures() -> Vec<Box<dyn Fixture>> {
    vec![
        from_string("empty.ts", "empty.ts", ""),
        from_file(
            "checker.ts",
            type_script_submodule_path().join("src/compiler/checker.ts"),
        ),
        from_file(
            "dom.generated.d.ts",
            type_script_submodule_path().join("src/lib/dom.generated.d.ts"),
        ),
        from_file(
            "Herebyfile.mjs",
            type_script_submodule_path().join("Herebyfile.mjs"),
        ),
        from_file(
            "jsxComplexSignatureHasApplicabilityError.tsx",
            type_script_submodule_path()
                .join("tests/cases/compiler/jsxComplexSignatureHasApplicabilityError.tsx"),
        ),
    ]
}

fn type_script_submodule_path() -> PathBuf {
    ts_repo::type_script_submodule_path().to_path_buf()
}
