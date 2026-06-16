use crate::{new_fourslash, TestingT};

pub fn test_call_hierarchy_across_project(t: &mut TestingT) {
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
		"bar.ts",
		"baz.ts"
	],
}
// @Filename: /projects/container/lib/index.ts
export function /*call*/createModelReference() {}
// @Filename: /projects/container/lib/bar.ts
import { createModelReference } from "./index";
function openElementsAtEditor() {
  createModelReference();
}
// @Filename: /projects/container/lib/baz.ts
import { createModelReference } from "./index";
function registerDefaultLanguageCommand() {
  createModelReference();
}
// @Filename: /projects/container/exec/tsconfig.json
{
	"files": ["./index.ts"],
	"references": [
		{ "path": "../lib" },
	],
}
// @Filename: /projects/container/exec/index.ts
import { createModelReference } from "../lib";
function openElementsAtEditor1() {
  createModelReference();
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
import { createModelReference } from "../lib";
function openElementsAtEditor2() {
  createModelReference();
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
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "call");
    // Open temp file and verify all projects alive
    f.go_to_marker(t, "temp");

    // Ref projects are loaded after as part of this command
    f.go_to_marker(t, "call");
    f.verify_baseline_call_hierarchy(t);

    // Open temp file and verify all projects alive
    f.close_file_of_marker(t, "temp");
    f.go_to_marker(t, "temp");

    // Close all files and open temp file, only inferred project should be alive
    f.close_file_of_marker(t, "call");
    f.close_file_of_marker(t, "temp");
    f.go_to_marker(t, "temp");
    done();
}

