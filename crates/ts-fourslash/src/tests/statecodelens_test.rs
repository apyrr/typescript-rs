use crate::{new_fourslash, CodeLensUserPreferences, TestingT, UserPreferences};

pub fn test_code_lens_across_projects(t: &mut TestingT) {
    let content = r#"
// @stateBaseline: true
// @Filename: /projects/temp/temp.ts
/*temp*/let x = 10
// @Filename: /projects/temp/tsconfig.json
{}
// @Filename: /projects/container/lib/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
	},
	references: [],
	files: [
		"index.ts",
		"bar.ts"
	],
}
// @Filename: /projects/container/lib/index.ts
/*impl*/
export interface Pointable {
  getX(): number;
  getY(): number;
}
export const val = 42;
// @Filename: /projects/container/lib/bar.ts
import { Pointable } from "./index";
class Point implements Pointable {
  getX(): number {
    return 0;
  }
  getY(): number {
    return 0;
  }
}
// @Filename: /projects/container/exec/tsconfig.json
{
	"files": ["./index.ts"],
	"references": [
		{ "path": "../lib" },
	],
}
// @Filename: /projects/container/exec/index.ts
import { Pointable } from "../lib";
class Point1 implements Pointable {
  getX(): number {
    return 0;
  }
  getY(): number {
    return 0;
  }
}
// @Filename: /projects/container/compositeExec/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
	},
	"files": ["./index.ts"],
	"references": [
		{ "path": "../lib" },
	],
}
// @Filename: /projects/container/compositeExec/index.ts
import { Pointable } from "../lib";
class Point2 implements Pointable {
  getX(): number {
    return 0;
  }
  getY(): number {
    return 0;
  }
}
// @Filename: /projects/container/tsconfig.json
{
	"files": [],
	"include": [],
	"references": [
		{ "path": "./exec" },
		{ "path": "./compositeExec" },
	],
}
// @Filename: /projects/container/tsconfig.json
{
	"files": [],
	"include": [],
	"references": [
		{ "path": "./exec" },
		{ "path": "./compositeExec" },
	],
}
// @Filename: /projects/container/tsconfig.json
{
	"files": [],
	"include": [],
	"references": [
		{ "path": "./exec" },
		{ "path": "./compositeExec" },
	],
}
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "impl");
    // Open temp file and verify all projects alive
    f.go_to_marker(t, "temp");

    // Ref projects are loaded after as part of this command
    f.verify_baseline_code_lens(
        t,
        Some(UserPreferences {
            code_lens: CodeLensUserPreferences {
                references_code_lens_enabled: Some(true),
                references_code_lens_show_on_all_functions: Some(true),
                implementations_code_lens_enabled: Some(true),
                implementations_code_lens_show_on_interface_methods: Some(true),
                implementations_code_lens_show_on_all_class_methods: Some(true),
            },
            ..UserPreferences::default()
        }),
    );

    // Open temp file and verify all projects alive
    f.close_file_of_marker(t, "temp");
    f.go_to_marker(t, "temp");

    // Close all files and open temp file, only inferred project should be alive
    f.close_file_of_marker(t, "impl");
    f.close_file_of_marker(t, "temp");
    f.go_to_marker(t, "temp");
    done();
}

pub fn test_code_lens_on_function_across_projects1(t: &mut TestingT) {
    let content = r#"
// @filename: ./a/tsconfig.json
{
  "compilerOptions": {
	"composite": true,
	"declaration": true,
	"declarationMaps": true,
	"outDir": "./dist",
	"rootDir": "src"
  },
  "include": ["./src"]
}

// @filename: ./a/src/foo.ts
export function aaa() {}
aaa();

// @filename: ./b/tsconfig.json
{
  "compilerOptions": {
	"composite": true,
	"declaration": true,
	"declarationMaps": true,
	"outDir": "./dist",
	"rootDir": "src"
  },
  "references": [{ "path": "../a" }],
  "include": ["./src"]
}

// @filename: ./b/src/bar.ts
import * as foo from '../../a/dist/foo.js';
foo.aaa();
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    f.verify_baseline_code_lens(
        t,
        Some(UserPreferences {
            code_lens: CodeLensUserPreferences {
                references_code_lens_enabled: Some(true),
                ..CodeLensUserPreferences::default()
            },
            ..UserPreferences::default()
        }),
    );
    done();
}

