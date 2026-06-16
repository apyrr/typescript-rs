use crate::{new_fourslash, TestingT};

pub fn test_call_hierarchy_incoming_calls_no_crash_array_push(t: &mut TestingT) {
    let content = r#"function splitNames(name: string) {
  return (name || "").split(",").filter(Boolean);
}

async function trim(packageNames: string[]) {
  const nameOrPkgs = packageNames.filter(Boolean);
  const names = [];
  for (const nameOrPkg of nameOrPkgs) {
    try {
      names./*push*/push(nameOrPkg);
    } catch (error) {
    }
  }
  return names;
}
	"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "push");
    f.verify_baseline_call_hierarchy(t);
    done();
}

