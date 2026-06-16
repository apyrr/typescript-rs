use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub const LOADER_SCRIPT: &str = r#"import script from "./script.mjs";
process.stdout.write(JSON.stringify(await script(...process.argv.slice(2))));"#;

pub fn eval_node_script(
    script: &str,
    dir: impl AsRef<Path>,
    args: &[String],
) -> Result<String, String> {
    eval_node_script_with_loader(script, LOADER_SCRIPT, dir, args)
}

pub fn eval_node_script_with_ts(
    script: &str,
    dir: impl AsRef<Path>,
    args: &[String],
) -> Result<String, String> {
    let ts_src = normalize_path(
        root_path()
            .join("node_modules")
            .join("typescript")
            .join("lib")
            .join("typescript.js"),
    );
    let ts_src = if ts_src.starts_with('/') {
        format!("file://{ts_src}")
    } else {
        format!("file:///{ts_src}")
    };
    let loader = format!(
        r#"import script from "./script.mjs";
import * as ts from "{ts_src}";
process.stdout.write(JSON.stringify(await script(ts, ...process.argv.slice(2))));"#
    );
    eval_node_script_with_loader(script, &loader, dir, args)
}

pub fn skip_if_no_node_js() -> Result<(), String> {
    if get_node_exe().is_empty() {
        Err("Node.js not found".to_string())
    } else {
        Ok(())
    }
}

pub fn eval_node_script_with_loader(
    script: &str,
    loader: &str,
    dir: impl AsRef<Path>,
    args: &[String],
) -> Result<String, String> {
    let exe = get_node_exe();
    if exe.is_empty() {
        return Err("Node.js not found".to_string());
    }
    let dir = dir.as_ref();
    fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    let script_path = dir.join("script.mjs");
    fs::write(&script_path, script).map_err(|err| err.to_string())?;
    let loader_path = dir.join("loader.mjs");
    fs::write(&loader_path, loader).map_err(|err| err.to_string())?;

    let output = Command::new(exe)
        .arg(&loader_path)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "failed to run node: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn get_node_exe() -> String {
    static NODE: OnceLock<String> = OnceLock::new();
    NODE.get_or_init(|| find_on_path("node").unwrap_or_default())
        .clone()
}

fn find_on_path(exe_name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(exe_name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

fn root_path() -> PathBuf {
    PathBuf::from(".")
}

fn normalize_path(path: PathBuf) -> String {
    path.to_string_lossy().replace('\\', "/")
}
