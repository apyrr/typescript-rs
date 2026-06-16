#![forbid(unsafe_code)]
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

static ROOT_PATH: OnceLock<PathBuf> = OnceLock::new();
static WORKSPACE_ROOT_PATH: OnceLock<PathBuf> = OnceLock::new();
static TYPE_SCRIPT_SUBMODULE_PATH: OnceLock<PathBuf> = OnceLock::new();
static BASELINE_OUTPUT_PATH: OnceLock<PathBuf> = OnceLock::new();
static TEST_DATA_PATH: OnceLock<PathBuf> = OnceLock::new();
static TYPE_SCRIPT_SUBMODULE_EXISTS: OnceLock<bool> = OnceLock::new();

pub fn root_path() -> &'static Path {
    ROOT_PATH.get_or_init(|| {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut dir = manifest_dir.as_path();
        loop {
            let vendored_go_root = dir.join("vendor").join("typescript-go");
            if vendored_go_root.join("go.mod").is_file() {
                return vendored_go_root;
            }
            let Some(parent) = dir.parent() else {
                break;
            };
            dir = parent;
        }

        let filename = std::env::current_exe().unwrap_or_else(|err| {
            panic!("could not get current filename: {err}");
        });
        let filename_string = filename.to_string_lossy();
        if filename_string.starts_with("github.com/") {
            panic!("repo root cannot be found when built with -trimpath");
        }

        if !filename.is_absolute() {
            panic!("{} is not an absolute path", filename.display());
        }

        let mut dir = filename.parent().unwrap_or_else(|| {
            panic!("could not get parent directory for {}", filename.display());
        });
        loop {
            if dir.join("go.mod").is_file() {
                return dir.to_owned();
            }
            let Some(parent) = dir.parent() else {
                break;
            };
            dir = parent;
        }

        panic!("could not find go.mod above {}", filename.display())
    })
}

pub fn workspace_root_path() -> &'static Path {
    WORKSPACE_ROOT_PATH.get_or_init(|| {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut dir = manifest_dir.as_path();
        loop {
            if dir.join("Cargo.toml").is_file() && dir.join("vendor").join("typescript-go").is_dir()
            {
                return dir.to_owned();
            }
            let Some(parent) = dir.parent() else {
                break;
            };
            dir = parent;
        }

        root_path()
            .parent()
            .and_then(Path::parent)
            .unwrap_or_else(root_path)
            .to_owned()
    })
}

pub fn type_script_submodule_path() -> &'static Path {
    TYPE_SCRIPT_SUBMODULE_PATH.get_or_init(|| {
        workspace_root_path()
            .join("vendor")
            .join("typescript-go")
            .join("_submodules")
            .join("TypeScript")
    })
}

pub fn baseline_output_path() -> &'static Path {
    BASELINE_OUTPUT_PATH.get_or_init(|| {
        workspace_root_path()
            .join("target")
            .join("tsgo")
            .join("baselines")
    })
}

pub fn test_data_path() -> &'static Path {
    TEST_DATA_PATH.get_or_init(|| root_path().join("testdata"))
}

pub fn type_script_submodule_exists() -> bool {
    *TYPE_SCRIPT_SUBMODULE_EXISTS.get_or_init(|| {
        let p = type_script_submodule_path().join("tests").join("cases");
        match std::fs::metadata(&p) {
            Ok(metadata) => metadata.is_dir(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
            Err(err) => panic!("{err}"),
        }
    })
}

pub trait SkippableTest {
    fn helper(&mut self);
    fn skipf(&mut self, message: String);
}

pub fn skip_if_no_type_script_submodule_for(t: &mut dyn SkippableTest) {
    t.helper();
    if !type_script_submodule_exists() {
        t.skipf("TypeScript submodule does not exist".to_string());
    }
}

pub fn skip_if_no_type_script_submodule() -> bool {
    !type_script_submodule_exists()
}
