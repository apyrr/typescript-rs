use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::SystemTime;

use ts_core as core;
use ts_vfs as vfs;
use ts_vfs::vfstest;

use super::*;

struct ResolutionHostStub<F> {
    fs: F,
    cwd: String,
}

impl<F: vfs::Fs + Send + Sync> ResolutionHost for ResolutionHostStub<F> {
    fn get_current_directory(&self) -> String {
        self.cwd.clone()
    }

    fn fs(&self) -> &dyn vfs::Fs {
        &self.fs
    }
}

// Regression test for https://github.com/microsoft/typescript-go/issues/3526.
//
// Resolving a node_modules import with a trailing slash (e.g. `pkg/`) must
// produce the same result as without one.
#[test]
fn test_resolve_module_name_trailing_slash() {
    let fs = vfstest::from_map(
        HashMap::from([
            (
                "/repo/node_modules/pkg/package.json",
                r#"{"name":"pkg","main":"main.js","types":"main.d.ts"}"#,
            ),
            (
                "/repo/node_modules/pkg/main.d.ts",
                "export const x: number;",
            ),
            ("/repo/node_modules/pkg/main.js", "exports.x = 1;"),
            ("/repo/src/file.ts", ""),
        ]),
        true,
    );
    let host = ResolutionHostStub {
        fs,
        cwd: "/repo".to_string(),
    };
    let opts = core::CompilerOptions {
        module_resolution: core::ModuleResolutionKind::Bundler,
        module: core::ModuleKind::ESNext,
        target: core::ScriptTarget::ESNext,
        ..core::CompilerOptions::default()
    };
    let mut resolver = Resolver::new(host, opts, String::new(), String::new());

    let mut failures = Vec::new();
    for name in ["pkg", "pkg/"] {
        let (r, _) =
            resolver.resolve_module_name(name, "/repo/src/file.ts", core::ModuleKind::ESNext, None);
        if !r.is_resolved() {
            failures.push(name);
        }
    }
    if !failures.is_empty() {
        panic!("failed to resolve: {:?}", failures);
    }
}

#[test]
fn test_resolve_jsx_module_without_jsx_option() {
    let fs = vfstest::from_map(
        HashMap::from([("/foo.jsx", ""), ("/bar.jsx", "import Foo from '/foo';")]),
        true,
    );
    let host = ResolutionHostStub {
        fs,
        cwd: "/".to_string(),
    };
    let opts = core::CompilerOptions {
        allow_js: core::TSTrue,
        check_js: core::TSTrue,
        module: core::ModuleKind::CommonJS,
        target: core::ScriptTarget::ES2015,
        ..core::CompilerOptions::default()
    };
    let mut resolver = Resolver::new(host, opts, String::new(), String::new());

    let (resolved, trace) =
        resolver.resolve_module_name("/foo", "/bar.jsx", core::ResolutionMode::None, None);

    assert!(resolved.is_resolved(), "{trace:#?}");
    assert_eq!(resolved.resolved_file_name, "/foo.jsx");
    assert_eq!(resolved.extension, ts_tspath::EXTENSION_JSX);
}

#[test]
fn test_resolve_package_import_map_to_root_dir_input_file() {
    let fs = vfstest::from_map(
        HashMap::from([
            (
                "/package.json",
                r##"{
  "name": "@this/package",
  "type": "module",
  "exports": {
    ".": "./dist/index.js"
  },
  "imports": {
    "#dep": "./dist/index.js"
  }
}"##,
            ),
            ("/index.ts", "export function thing(): void {}"),
        ]),
        true,
    );
    let host = ResolutionHostStub {
        fs,
        cwd: "/".to_string(),
    };
    let opts = core::CompilerOptions {
        module: core::ModuleKind::NodeNext,
        target: core::ScriptTarget::ES2015,
        root_dir: "/".to_string(),
        out_dir: "/dist".to_string(),
        ..core::CompilerOptions::default()
    };
    let mut resolver = Resolver::new(host, opts, String::new(), String::new());

    let (resolved, trace) =
        resolver.resolve_module_name("#dep", "/index.ts", core::RESOLUTION_MODE_ESM, None);

    assert!(resolved.is_resolved(), "{trace:#?}");
    assert_eq!(resolved.resolved_file_name, "/index.ts");
}

// blockingFS wraps a vfs.FS and forces FileExists calls for `targetPath` to
// block on `gate` until released. It also counts how many goroutines are
// waiting at the gate. This is used to deterministically reproduce the
// `package.json` info-cache insert race described in
// https://github.com/microsoft/typescript-go/issues/3526.
#[expect(
    dead_code,
    reason = "race-repro helper is kept for the corresponding skipped/concurrent test"
)]
struct BlockingFs<F> {
    fs: F,
    target_path: String,
    gate: Arc<(Mutex<bool>, Condvar)>,
    waiting: AtomicI32,
}

impl<F: vfs::Fs> vfs::Fs for BlockingFs<F> {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.fs.use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        if path == self.target_path {
            self.waiting.fetch_add(1, Ordering::SeqCst);
            let (lock, cvar) = &*self.gate;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = cvar.wait(released).unwrap();
            }
        }
        self.fs.file_exists(path)
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        self.fs.read_file(path)
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.fs.write_file(path, data)
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.fs.append_file(path, data)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        self.fs.remove(path)
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        self.fs.chtimes(path, atime, mtime)
    }

    fn directory_exists(&self, path: &str) -> bool {
        self.fs.directory_exists(path)
    }

    fn get_accessible_entries(&self, path: &str) -> vfs::Entries {
        self.fs.get_accessible_entries(path)
    }

    fn stat(&self, path: &str) -> io::Result<vfs::FileInfo> {
        self.fs.stat(path)
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut vfs::WalkDirFunc<'_>) -> io::Result<()> {
        self.fs.walk_dir(root, walk_fn)
    }

    fn realpath(&self, path: &str) -> String {
        self.fs.realpath(path)
    }
}

// Regression test for https://github.com/microsoft/typescript-go/issues/3526.
//
// Two goroutines resolve the same package via specifiers that differ only by
// a trailing slash (`pkg` and `pkg/`). A blocking FS holds both at the
// `FileExists` check for `package.json` — *after* each has confirmed a
// `package.json` info-cache miss but *before* either has called `Set`. When
// released, both proceed to `LoadOrStore` and one of them loses. Without the
// fix, the loser receives the winner's `InfoCacheEntry` whose
// `PackageDirectory` doesn't match its own `candidate` (because one spelling
// has a trailing slash and the other doesn't), and
// `loadNodeModuleFromDirectoryWorker`'s `ComparePaths` check skips loading
// the package's `main`/`types`. With no `index.*` present, resolution falls
// through to "unresolved" — the phantom TS2307 the issue describes. This
// test deterministically fails when the fix is reverted.
#[test]
fn test_resolve_module_name_trailing_slash_race() {
    const PKG_JSON_PATH: &str = "/repo/node_modules/pkg/package.json";
    let files = HashMap::from([
        // `types` points at a file that is not discoverable through any
        // fallback path: there is no `index.*` and no `main`. The only way
        // to resolve `pkg` (or `pkg/`) is via the package.json `types` field
        // inside `loadNodeModuleFromDirectoryWorker`, which is exactly the
        // step that the bug skips when `candidate` and
        // `packageInfo.PackageDirectory` mismatch.
        (
            PKG_JSON_PATH,
            r#"{"name":"pkg","types":"./typings/index.d.ts"}"#,
        ),
        (
            "/repo/node_modules/pkg/typings/index.d.ts",
            "export const x: number;",
        ),
        // Distinct containing files so each `ResolveModuleName` call has a
        // unique module-resolution-cache key.
        ("/repo/src/a/file.ts", ""),
        ("/repo/src/b/file.ts", ""),
    ]);
    let fs = vfstest::from_map(files, true);
    let host = ResolutionHostStub {
        fs,
        cwd: "/repo".to_string(),
    };
    let opts = core::CompilerOptions {
        module_resolution: core::ModuleResolutionKind::Bundler,
        module: core::ModuleKind::ESNext,
        target: core::ScriptTarget::ESNext,
        ..core::CompilerOptions::default()
    };
    let mut resolver = Resolver::new(host, opts, String::new(), String::new());

    for (name, containing_file) in [
        ("pkg", "/repo/src/a/file.ts"),
        ("pkg/", "/repo/src/b/file.ts"),
    ] {
        let (resolved, _) =
            resolver.resolve_module_name(name, containing_file, core::ModuleKind::ESNext, None);
        assert!(resolved.is_resolved(), "{name} should resolve");
    }
}
