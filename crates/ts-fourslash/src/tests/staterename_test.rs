use crate::{new_fourslash, TestingT};

pub fn test_rename_ancestor_project_ref_mangement(t: &mut TestingT) {
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
	],
}
// @Filename: /projects/container/lib/index.ts
export const myConst = 30;
// @Filename: /projects/container/exec/tsconfig.json
{
	"files": ["./index.ts"],
	"references": [
		{ "path": "../lib" },
	],
}
// @Filename: /projects/container/exec/index.ts
import { myConst } from "../lib";
export function getMyConst() {
	return myConst;
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
import { /*find*/myConst } from "../lib";
export function getMyConst() {
	return myConst;
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
    f.go_to_marker(t, "find");
    // Open temp file and verify all projects alive
    f.go_to_marker(t, "temp");

    // Ref projects are loaded after as part of this command
    f.verify_baseline_rename(t, &["find".to_string()]);

    // Open temp file and verify all projects alive
    f.close_file_of_marker(t, "temp");
    f.go_to_marker(t, "temp");

    // Close all files and open temp file, only inferred project should be alive
    f.close_file_of_marker(t, "find");
    f.close_file_of_marker(t, "temp");
    f.go_to_marker(t, "temp");
    done();
}

pub fn test_rename_in_common_file(t: &mut TestingT) {
    let content = r#"
// @stateBaseline: true
// @Filename: /projects/a/a.ts
/*aTs*/import {C} from "./c/fc";
console.log(C)
// @Filename: /projects/a/tsconfig.json
{}
// @link:  /projects/c -> /projects/a/c
// @Filename: /projects/b/b.ts
/*bTs*/import {C} from "../c/fc";
console.log(C)
// @Filename: /projects/b/tsconfig.json
{}
// @link:  /projects/c -> /projects/b/c
// @Filename: /projects/c/fc.ts
export const /*find*/C = 42;
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "aTs");
    f.go_to_marker(t, "bTs");
    let find_marker = f.marker_by_name("find");
    let a_fc_marker = find_marker.maker_with_symlink("/projects/a/c/fc.ts".to_string());
    f.go_to_marker_or_range(t, a_fc_marker.clone().into());
    f.go_to_marker_or_range(
        t,
        find_marker
            .maker_with_symlink("/projects/b/c/fc.ts".to_string())
            .into(),
    );
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![a_fc_marker.into()]);
    done();
}

