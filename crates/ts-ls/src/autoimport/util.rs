use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ts_ast as ast;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_module as module;
use ts_modulespecifiers as modulespecifiers;
use ts_packagejson as packagejson;
use ts_tspath as tspath;
use ts_vfs as vfs;
use ts_vfs::wrapvfs;

use crate::autoimport::{ModuleId, RegistryCloneHost};

pub fn try_get_module_id_and_file_name_of_module_symbol(
    checker: &mut checker::Checker<'_, '_>,
    symbol: ast::SymbolIdentity,
) -> (ModuleId, String, bool) {
    let flags = checker
        .symbol_flags_public(symbol)
        .unwrap_or(ast::SYMBOL_FLAGS_NONE);
    let name = checker.symbol_name_public(symbol).unwrap_or_default();
    if flags & ast::SYMBOL_FLAGS_MODULE == 0 || !name.starts_with('"') {
        return (String::new(), String::new(), false);
    }

    let declarations = checker.collect_symbol_declarations_public(symbol);
    let mut decl = None;
    for declaration in declarations {
        let Some(source_file) = checker.try_source_file_for_node_public(declaration) else {
            continue;
        };
        let store = source_file.store();
        if !ast::is_module_augmentation_external(store, declaration)
            && !ast::is_global_scope_augmentation(store, declaration)
        {
            decl = Some(declaration);
            break;
        }
    }
    let Some(decl) = decl else {
        return (String::new(), String::new(), false);
    };
    let Some(source_file) = checker.try_source_file_for_node_public(decl) else {
        return (String::new(), String::new(), false);
    };
    let store = source_file.store();
    if store.kind(decl) == ast::Kind::SourceFile {
        let source_file = store.as_source_file(decl);
        return (source_file.path(), source_file.file_name(), true);
    }
    if let Some(name) = ast::module_string_literal_name(store, decl) {
        return (store.text(name), String::new(), true);
    }
    (String::new(), String::new(), false)
}

// word_indices splits an identifier into its constituent words based on camelCase and snake_case conventions
// by returning the starting byte indices of each word. The first index is always 0.
//   - CamelCase
//     ^    ^
//   - snake_case
//     ^     ^
//   - ParseURL
//     ^    ^
//   - __proto__
//     ^
pub fn word_indices(s: &str) -> Vec<usize> {
    let mut indices = Vec::new();
    for (byte_index, rune_value) in s.char_indices() {
        if byte_index == 0 {
            indices.push(byte_index);
            continue;
        }
        if rune_value == '_' {
            if byte_index + 1 < s.len() && s.as_bytes()[byte_index + 1] != b'_' {
                indices.push(byte_index + 1);
            }
            continue;
        }
        let previous = s[..byte_index].chars().next_back();
        let next = if rune_value.len_utf8() == 1 {
            s[byte_index + 1..].chars().next()
        } else {
            None
        };
        if rune_value.is_uppercase()
            && (previous.is_some_and(char::is_lowercase) || next.is_some_and(char::is_lowercase))
        {
            indices.push(byte_index);
        }
    }
    indices
}

pub fn get_package_names_in_node_modules(
    node_modules_dir: &str,
    fs: &dyn vfs::Fs,
) -> Result<ts_collections::Set<String>, std::io::Error> {
    let mut package_names = ts_collections::Set::default();
    if tspath::get_base_file_name(node_modules_dir) != "node_modules" {
        panic!("nodeModulesDir is not a node_modules directory");
    }
    if !fs.directory_exists(node_modules_dir) {
        return Err(std::io::Error::from(std::io::ErrorKind::NotFound));
    }
    let entries = fs.get_accessible_entries(node_modules_dir);
    for base_name in entries.directories {
        if base_name.starts_with('.') {
            continue;
        }
        if base_name.starts_with('@') {
            let scoped_dir_path = tspath::combine_paths(node_modules_dir, &[&base_name]);
            for scoped_package_dir_name in fs.get_accessible_entries(&scoped_dir_path).directories {
                let scoped_base_name = tspath::get_base_file_name(&scoped_package_dir_name);
                if base_name == "@types" {
                    package_names.add(module::get_package_name_from_types_package_name(
                        &tspath::combine_paths("@types", &[&scoped_base_name]),
                    ));
                } else {
                    package_names.add(tspath::combine_paths(&base_name, &[&scoped_base_name]));
                }
            }
            continue;
        }
        package_names.add(base_name);
    }
    Ok(package_names)
}

pub fn get_resolved_package_names(
    ctx: core::Context,
    program: &compiler::Program,
) -> Result<ts_collections::Set<String>, core::Error> {
    let raw_names = program.resolved_package_names_for_auto_imports();
    let unresolved_package_names = program.unresolved_package_names_for_auto_imports();

    // Normalize @types/ package names to their actual package names
    // (e.g., "@types/react" → "react"). ResolvedPackageNames can contain
    // @types names when the program resolves an import like "react" to
    // "@types/react/index.d.ts" via the PackageId.Name field.
    let mut resolved_package_names = ts_collections::new_set_with_size_hint(raw_names.len());
    for name in raw_names.keys().into_iter().flatten() {
        resolved_package_names.add(module::get_package_name_from_types_package_name(name));
    }

    for name in &program.compiler_options().types {
        if name != "*" {
            resolved_package_names.add(module::get_package_name_from_types_package_name(name));
        }
    }

    if unresolved_package_names.len() > 0 {
        let source_files = program.source_files_for_auto_imports();
        let Some(checker_file) = source_files.first() else {
            return Ok(resolved_package_names);
        };
        program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(&ctx),
            checker_file,
            |checker| {
                for name in unresolved_package_names.keys().into_iter().flatten() {
                    if let Some(symbol) = checker.try_find_ambient_module_public(name) {
                        let declarations = checker.collect_symbol_declarations_public(symbol);
                        let Some(declaration) = declarations.first() else {
                            continue;
                        };
                        let Some(store) = modulespecifiers::CheckerShape::source_file_store(
                            &*checker,
                            *declaration,
                        ) else {
                            continue;
                        };
                        if let Some(declaring_file) =
                            get_source_file_of_module_symbol(store, &declarations)
                        {
                            let package_name = modulespecifiers::get_package_name_from_directory(
                                &store.as_source_file(declaring_file).file_name(),
                            );
                            if !package_name.is_empty() {
                                resolved_package_names.add(
                                    module::get_package_name_from_types_package_name(&package_name),
                                );
                            }
                        }
                    }
                }
                Ok(())
            },
        )?;
    }
    Ok(resolved_package_names)
}

fn get_source_file_of_module_symbol(
    store: &ast::AstStore,
    declarations: &[ast::Node],
) -> Option<ast::Node> {
    declarations
        .iter()
        .find_map(|declaration| ast::get_source_file_of_node(store, Some(*declaration)))
}

// addProjectReferenceOutputMappings adds output .d.ts to source file mappings
// from a program's project references to the provided map.
// This is used during node_modules bucket building to redirect extraction
// from output files to source files when the output is from a project reference.
pub fn add_project_reference_output_mappings(
    program: &compiler::Program,
    result: &mut HashMap<tspath::Path, String>,
) {
    for (output_dts_path, source) in program.project_reference_output_mappings_for_auto_imports() {
        // Only add if not already present (first program wins)
        result.entry(output_dts_path).or_insert(source);
    }
}

// addPackageJsonDependencies adds all dependencies and peerDependencies from a package.json
// to the given set, canonicalizing @types package names to their base names.
pub fn add_package_json_dependencies(
    contents: &packagejson::PackageJson,
    deps: &mut ts_collections::Set<String>,
) {
    contents
        .fields
        .dependency_fields
        .range_dependencies(|name, _, field| {
            if name.is_empty() || name == "@types/" || name.starts_with('.') {
                return true;
            }
            if field == "dependencies" || field == "peerDependencies" {
                deps.add(module::get_package_name_from_types_package_name(name));
            }
            true
        });
}

// getPackageRealpathFuncs returns functions to transform between symlink and realpath for files within a package.
// It calls FS.Realpath once per package directory and uses prefix substitution for files within that directory,
// avoiding expensive realpath syscalls for each file. For files outside the package (e.g. re-exported
// dependencies reached through node_modules symlinks), it resolves the file's directory realpath once,
// finds the symlink boundary (the package root where the symlink lives), and caches that prefix mapping.
// All subsequent files under the same symlinked package directory use prefix substitution with no syscalls.
pub fn get_package_realpath_funcs(
    fs: vfs::FS,
    package_dir: &str,
) -> (
    Box<dyn Fn(&str) -> String + Send + Sync>,
    Box<dyn Fn(&str) -> String + Send + Sync>,
) {
    let real_package_dir = fs.realpath(package_dir);
    let is_symlinked = real_package_dir != package_dir;
    // Cache of package-directory-level symlink->realpath prefix mappings for
    // external packages encountered via re-exports. Keyed by the node_modules
    // package directory (e.g. "/app/node_modules/dep"), so all files under
    // that package reuse a single realpath lookup.
    let dir_cache = Arc::new(Mutex::new(HashMap::<String, String>::new()));

    let to_realpath = {
        let real_package_dir = real_package_dir.clone();
        let package_dir = package_dir.to_string();
        let dir_cache = Arc::clone(&dir_cache);
        let fs = fs.clone();
        move |file_name: &str| {
            // Fast path: files within the package use prefix substitution.
            if is_symlinked {
                if let Some(after) = file_name.strip_prefix(&package_dir) {
                    return format!("{real_package_dir}{after}");
                }
            }
            // Files outside the package (e.g. re-exports into symlinked deps):
            // find the node_modules package directory, resolve it once, and cache.
            let pkg_dir = module::parse_node_module_from_path(file_name, false);
            if pkg_dir.is_empty() {
                return file_name.to_string();
            }
            if let Some(real_dir) = dir_cache.lock().unwrap().get(&pkg_dir).cloned() {
                if real_dir == pkg_dir {
                    return file_name.to_string();
                }
                return format!("{real_dir}{}", &file_name[pkg_dir.len()..]);
            }
            let real_dir = fs.realpath(&pkg_dir);
            dir_cache
                .lock()
                .unwrap()
                .insert(pkg_dir.clone(), real_dir.clone());
            if real_dir == pkg_dir {
                return file_name.to_string();
            }
            format!("{real_dir}{}", &file_name[pkg_dir.len()..])
        }
    };

    let to_symlink = {
        let real_package_dir = real_package_dir.clone();
        let package_dir = package_dir.to_string();
        move |file_name: &str| {
            if !is_symlinked {
                return file_name.to_string();
            }
            // toSymlink only handles files within the package directory (reversing the
            // packageDir->realPackageDir substitution). It does not handle arbitrary external
            // paths; callers should only use it for files known to be within the package.
            if let Some(after) = file_name.strip_prefix(&real_package_dir) {
                return format!("{package_dir}{after}");
            }
            file_name.to_string()
        }
    };

    (Box::new(to_realpath), Box::new(to_symlink))
}

pub struct ResolutionHost {
    pub fs: Box<dyn vfs::Fs + Send + Sync>,
    pub current_directory: String,
}

impl module::ResolutionHost for ResolutionHost {
    fn get_current_directory(&self) -> String {
        self.current_directory.clone()
    }

    fn fs(&self) -> &dyn vfs::Fs {
        self.fs.as_ref()
    }
}

pub fn get_module_resolver(
    host: &dyn RegistryCloneHost,
    realpath: impl Fn(&str) -> String + Send + Sync + 'static,
    opts: module::ResolverOptions,
) -> module::Resolver {
    let rh = ResolutionHost {
        fs: Box::new(wrapvfs::wrap(
            RegistryCloneHost::fs(host),
            wrapvfs::Replacements {
                realpath: Some(Box::new(realpath)),
                ..Default::default()
            },
        )),
        current_directory: host.get_current_directory(),
    };
    module::new_resolver_with_options(rh, core::empty_compiler_options(), "", "", opts)
}
