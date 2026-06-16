#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports28_long_types() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports28_long_types(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports28_long_types(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
// @isolatedDeclarations: true
// @declaration: true
export const sessionLoader = {
    async loadSession() {
        if (Math.random() > 0.5) {
            return {
                PROP_1: {
                    name: false,
                },
                PROPERTY_2: {
                    name: 1,
                },
                PROPERTY_3: {
                    name: 1
                },
                PROPERTY_4: {
                    name: 315,
                },
            };
        }

        return {
            PROP_1: {
                name: false,
            },
            PROPERTY_2: {
                name: undefined,
            },
            PROPERTY_3: {
            },
            PROPERTY_4: {
                name: 576,
            },
        };
    },
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_available(t, Some(&vec!["Add return type 'Promise<{\n    PROP_1: {\n        name: boolean;\n    };\n    PROPERTY_2: {\n        name: number;\n    };\n    PROPERTY_3: {\n        name: number;\n    };\n    PROPE...'".to_string()]));
    f.verify_code_fix(t, VerifyCodeFixOptions {
    description: "Add return type 'Promise<{\n    PROP_1: {\n        name: boolean;\n    };\n    PROPERTY_2: {\n        name: number;\n    };\n    PROPERTY_3: {\n        name: number;\n    };\n    PROPE...'".to_string(),
    new_file_content: r"export const sessionLoader = {
    async loadSession(): Promise<{
        PROP_1: {
            name: boolean;
        };
        PROPERTY_2: {
            name: number;
        };
        PROPERTY_3: {
            name: number;
        };
        PROPERTY_4: {
            name: number;
        };
    } | {
        PROP_1: {
            name: boolean;
        };
        PROPERTY_2: {
            name: any;
        };
        PROPERTY_3: {
            name?: undefined;
        };
        PROPERTY_4: {
            name: number;
        };
    }> {
        if (Math.random() > 0.5) {
            return {
                PROP_1: {
                    name: false,
                },
                PROPERTY_2: {
                    name: 1,
                },
                PROPERTY_3: {
                    name: 1
                },
                PROPERTY_4: {
                    name: 315,
                },
            };
        }

        return {
            PROP_1: {
                name: false,
            },
            PROPERTY_2: {
                name: undefined,
            },
            PROPERTY_3: {
            },
            PROPERTY_4: {
                name: 576,
            },
        };
    },
};".to_string(),
    new_range_content: String::new(),
    index: 0,
    apply_changes: false,
    user_preferences: None,
});
    done();
}
