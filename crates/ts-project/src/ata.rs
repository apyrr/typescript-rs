use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI32, Ordering},
};

use ts_collections as collections;
use ts_core as core;
use ts_module as module;
use ts_packagejson as packagejson;
use ts_semver as semver;
use ts_tspath as tspath;
use ts_vfs as vfs;

use crate::logging::{LogTree, Logger};

#[derive(Clone)]
pub struct TypingsInfo {
    pub type_acquisition: Option<core::TypeAcquisition>,
    pub compiler_options: core::CompilerOptions,
    pub unresolved_imports: Option<collections::Set<String>>,
}

impl TypingsInfo {
    pub fn equals(&self, other: TypingsInfo) -> bool {
        self.type_acquisition == other.type_acquisition
            && self.compiler_options.allow_js == other.compiler_options.allow_js
            && self.unresolved_imports == other.unresolved_imports
    }
}

pub trait NpmExecutor: Send + Sync {
    fn npm_install(&self, cwd: &str, args: &[String]) -> Result<Vec<u8>, String>;
}

#[derive(Clone)]
pub struct CachedTyping {
    pub typings_location: String,
    pub version: semver::Version,
}

#[derive(Clone)]
pub struct TypingsInstallerOptions {
    pub typings_location: String,
    pub throttle_limit: usize,
}

pub struct TypingsInstallRequest {
    pub project_id: tspath::Path,
    pub typings_info: TypingsInfo,
    pub file_names: Vec<String>,
    pub project_root_path: String,
    pub compiler_options: core::CompilerOptions,
    pub current_directory: String,
    pub get_script_kind: fn(&str) -> core::ScriptKind,
    pub fs: Arc<dyn vfs::Fs + Send + Sync>,
    pub logger: Option<Arc<LogTree>>,
}

pub struct TypingsInstallResult {
    pub typings_files: Vec<String>,
    pub files_to_watch: Vec<String>,
}

pub struct TypingsInstaller {
    typings_location: String,
    npm_executor: Box<dyn NpmExecutor>,
    throttle_limit: usize,
    package_name_to_typing_location: Mutex<HashMap<String, CachedTyping>>,
    missing_typings_set: Mutex<HashMap<String, bool>>,
    types_registry: Mutex<HashMap<String, HashMap<String, String>>>,
    install_run_count: AtomicI32,
    initialized: Mutex<bool>,
}

pub fn new_typings_installer(
    options: TypingsInstallerOptions,
    npm_executor: Box<dyn NpmExecutor>,
) -> TypingsInstaller {
    TypingsInstaller {
        typings_location: options.typings_location,
        npm_executor,
        throttle_limit: options.throttle_limit,
        package_name_to_typing_location: Mutex::new(HashMap::new()),
        missing_typings_set: Mutex::new(HashMap::new()),
        types_registry: Mutex::new(HashMap::new()),
        install_run_count: AtomicI32::new(0),
        initialized: Mutex::new(false),
    }
}

impl TypingsInstaller {
    pub fn is_known_types_package_name(
        &self,
        project_id: tspath::Path,
        name: &str,
        fs: Arc<dyn vfs::Fs + Send + Sync>,
        logger: Option<Arc<LogTree>>,
    ) -> bool {
        let (validation_result, _, _) = validate_package_name(name);
        if validation_result != NameValidationResult::NameOk {
            return false;
        }
        self.init(&project_id.to_string(), fs, logger.as_deref());
        self.types_registry
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .contains_key(name)
    }

    pub fn install_typings(
        &mut self,
        request: TypingsInstallRequest,
    ) -> Result<TypingsInstallResult, String> {
        let mut result = self.discover_and_install_typings(request)?;
        result.typings_files.sort();
        result.files_to_watch.sort();
        Ok(result)
    }

    fn discover_and_install_typings(
        &mut self,
        request: TypingsInstallRequest,
    ) -> Result<TypingsInstallResult, String> {
        self.init(
            &request.project_id.to_string(),
            request.fs.clone(),
            request.logger.as_deref(),
        );

        let (cached_typing_paths, new_typing_names, files_to_watch) = self.discover_typings(
            request.fs.clone(),
            request.logger.as_deref(),
            &request.typings_info,
            &request.file_names,
            &request.project_root_path,
        );

        let request_id = self.install_run_count.fetch_add(1, Ordering::SeqCst) + 1;
        if !new_typing_names.is_empty() {
            let filtered_typings = self.filter_typings(
                &request.project_id,
                request.logger.as_deref(),
                &new_typing_names,
            );
            if !filtered_typings.is_empty() {
                let typings_files = self.install_typings_worker(
                    &request,
                    request_id,
                    cached_typing_paths,
                    filtered_typings,
                )?;
                return Ok(TypingsInstallResult {
                    typings_files,
                    files_to_watch,
                });
            }
            log(
                request.logger.as_deref(),
                "ATA:: All typings are known to be missing or invalid - no need to install more typings",
            );
        } else {
            log(
                request.logger.as_deref(),
                "ATA:: No new typings were requested as a result of typings discovery",
            );
        }

        Ok(TypingsInstallResult {
            typings_files: cached_typing_paths,
            files_to_watch,
        })
    }

    fn discover_typings(
        &self,
        fs: Arc<dyn vfs::Fs + Send + Sync>,
        logger: Option<&LogTree>,
        typings_info: &TypingsInfo,
        file_names: &[String],
        project_root_path: &str,
    ) -> (Vec<String>, Vec<String>, Vec<String>) {
        let mut inferred_typings: HashMap<String, String> = HashMap::new();
        let file_names = file_names
            .iter()
            .filter(|file_name| tspath::has_js_file_extension(file_name))
            .cloned()
            .collect::<Vec<_>>();

        if let Some(type_acquisition) = &typings_info.type_acquisition {
            add_inferred_typings(
                logger,
                &mut inferred_typings,
                &type_acquisition.include,
                "Explicitly included types",
            );
            if typings_info.compiler_options.types.is_empty() {
                let mut possible_search_dirs = HashMap::new();
                for file_name in &file_names {
                    possible_search_dirs.insert(tspath::get_directory_path(file_name), true);
                }
                possible_search_dirs.insert(project_root_path.to_string(), true);
                let mut files_to_watch = Vec::new();
                for search_dir in possible_search_dirs.keys() {
                    files_to_watch = add_typing_names_and_get_files_to_watch(
                        fs.as_ref(),
                        logger,
                        &mut inferred_typings,
                        files_to_watch,
                        search_dir,
                        "bower.json",
                        "bower_components",
                    );
                    files_to_watch = add_typing_names_and_get_files_to_watch(
                        fs.as_ref(),
                        logger,
                        &mut inferred_typings,
                        files_to_watch,
                        search_dir,
                        "package.json",
                        "node_modules",
                    );
                }
                if !type_acquisition
                    .disable_filename_based_type_acquisition
                    .is_true()
                {
                    get_typing_names_from_source_file_names(
                        logger,
                        &mut inferred_typings,
                        &file_names,
                    );
                }
                self.add_unresolved_import_typings(logger, &mut inferred_typings, typings_info);
                for exclude_typing_name in &type_acquisition.exclude {
                    inferred_typings.remove(exclude_typing_name);
                    logf(
                        logger,
                        format!(
                            "ATA:: Typing for {exclude_typing_name} is in exclude list, will be ignored."
                        ),
                    );
                }
                let (cached_typing_paths, new_typing_names) =
                    self.split_cached_and_new_typings(&mut inferred_typings);
                logf(
                    logger,
                    format!(
                        "ATA:: Finished typings discovery: cachedTypingsPaths: {:?} newTypingNames: {:?}, filesToWatch {:?}",
                        cached_typing_paths, new_typing_names, files_to_watch
                    ),
                );
                return (cached_typing_paths, new_typing_names, files_to_watch);
            }
        }

        self.add_unresolved_import_typings(logger, &mut inferred_typings, typings_info);
        let (cached_typing_paths, new_typing_names) =
            self.split_cached_and_new_typings(&mut inferred_typings);
        (cached_typing_paths, new_typing_names, Vec::new())
    }

    fn add_unresolved_import_typings(
        &self,
        logger: Option<&LogTree>,
        inferred_typings: &mut HashMap<String, String>,
        typings_info: &TypingsInfo,
    ) {
        let mut modules = Vec::new();
        if let Some(unresolved_imports) = &typings_info.unresolved_imports {
            if let Some(keys) = unresolved_imports.keys() {
                modules.extend(
                    keys.iter()
                        .map(|module| core::non_relative_module_name_for_typing_cache(module)),
                );
            }
            modules.sort();
            modules.dedup();
        }
        add_inferred_typings(
            logger,
            inferred_typings,
            &modules,
            "Inferred typings from unresolved imports",
        );
    }

    fn split_cached_and_new_typings(
        &self,
        inferred_typings: &mut HashMap<String, String>,
    ) -> (Vec<String>, Vec<String>) {
        let cache = self
            .package_name_to_typing_location
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let registry = self
            .types_registry
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        for (name, typing) in cache.iter() {
            if inferred_typings
                .get(name)
                .is_some_and(|value| value.is_empty())
                && registry
                    .get(name)
                    .is_some_and(|entry| is_typing_up_to_date(typing, entry))
            {
                inferred_typings.insert(name.clone(), typing.typings_location.clone());
            }
        }
        let mut cached_typing_paths = Vec::new();
        let mut new_typing_names = Vec::new();
        for (typing, inferred) in inferred_typings {
            if inferred.is_empty() {
                new_typing_names.push(typing.clone());
            } else {
                cached_typing_paths.push(inferred.clone());
            }
        }
        (cached_typing_paths, new_typing_names)
    }

    fn install_typings_worker(
        &self,
        request: &TypingsInstallRequest,
        request_id: i32,
        currently_cached_typings: Vec<String>,
        filtered_typings: Vec<String>,
    ) -> Result<Vec<String>, String> {
        let scoped_typings = filtered_typings
            .iter()
            .map(|package_name| format!("@types/{package_name}@{TS_VERSION_TO_USE}"))
            .collect::<Vec<_>>();

        if self.install_worker(request, request_id, &scoped_typings) {
            logf(
                request.logger.as_deref(),
                format!("ATA:: Installed typings {:?}", scoped_typings),
            );
            let mut installed_typing_files = Vec::new();
            let mut resolver = module::new_resolver(
                AtaResolutionHost {
                    current_directory: request.current_directory.clone(),
                    fs: request.fs.clone(),
                },
                core::CompilerOptions {
                    module_resolution: core::ModuleResolutionKind::NodeNext,
                    ..core::CompilerOptions::default()
                },
                "",
                "",
            );
            for package_name in &filtered_typings {
                let typing_file = self.typing_to_file_name(&mut resolver, package_name);
                if typing_file.is_empty() {
                    logf(
                        request.logger.as_deref(),
                        format!("ATA:: Failed to find typing file for package '{package_name}'"),
                    );
                    self.missing_typings_set
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .insert(package_name.clone(), true);
                    continue;
                }

                let registry = self
                    .types_registry
                    .lock()
                    .unwrap_or_else(|err| err.into_inner());
                let Some(dist_tags) = registry.get(package_name) else {
                    continue;
                };
                let use_version = dist_tags
                    .get(&format!("ts{}", core::version_major_minor()))
                    .or_else(|| dist_tags.get("latest"))
                    .cloned()
                    .unwrap_or_default();
                drop(registry);
                let new_version = semver::must_parse(&use_version);
                self.package_name_to_typing_location
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .insert(
                        package_name.clone(),
                        CachedTyping {
                            typings_location: typing_file.clone(),
                            version: new_version,
                        },
                    );
                installed_typing_files.push(typing_file);
            }
            logf(
                request.logger.as_deref(),
                format!("ATA:: Installed typing files {:?}", installed_typing_files),
            );
            let mut result = currently_cached_typings;
            result.extend(installed_typing_files);
            return Ok(result);
        }

        logf(
            request.logger.as_deref(),
            format!(
                "ATA:: install request failed, marking packages as missing to prevent repeated requests: {:?}",
                filtered_typings
            ),
        );
        let mut missing = self
            .missing_typings_set
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        for typing in filtered_typings {
            missing.insert(typing, true);
        }
        Err("npm install failed".to_string())
    }

    fn install_worker(
        &self,
        request: &TypingsInstallRequest,
        request_id: i32,
        package_names: &[String],
    ) -> bool {
        logf(
            request.logger.as_deref(),
            format!(
                "ATA:: #{request_id} with cwd: {} arguments: {:?}",
                self.typings_location, package_names
            ),
        );
        let mut current_command_start = 0;
        let mut current_command_end = 0;
        let mut current_command_size = 100;
        for package_name in package_names {
            current_command_size += package_name.len() + 1;
            if current_command_size < 8000 {
                current_command_end += 1;
            } else {
                if !self.install_package_batch(
                    request,
                    &package_names[current_command_start..current_command_end],
                ) {
                    return false;
                }
                current_command_start = current_command_end;
                current_command_size = 100 + package_name.len() + 1;
                current_command_end += 1;
            }
        }
        if current_command_start < package_names.len()
            && !self.install_package_batch(
                request,
                &package_names[current_command_start..current_command_end],
            )
        {
            return false;
        }
        logf(
            request.logger.as_deref(),
            format!("TI:: npm install #{request_id} completed"),
        );
        true
    }

    fn install_package_batch(
        &self,
        request: &TypingsInstallRequest,
        package_names: &[String],
    ) -> bool {
        let mut npm_args = vec!["install".to_string(), "--ignore-scripts".to_string()];
        npm_args.extend(package_names.iter().cloned());
        npm_args.push("--save-dev".to_string());
        npm_args.push(format!(
            "--user-agent=\"typesInstaller/{}\"",
            core::version()
        ));
        match self
            .npm_executor
            .npm_install(&self.typings_location, &npm_args)
        {
            Ok(_) => true,
            Err(err) => {
                logf(request.logger.as_deref(), format!("ATA:: Output is: {err}"));
                false
            }
        }
    }

    fn filter_typings(
        &self,
        _project_id: &tspath::Path,
        logger: Option<&LogTree>,
        typings_to_install: &[String],
    ) -> Vec<String> {
        let mut result = Vec::new();
        for typing in typings_to_install {
            let typing_key = module::mangle_scoped_package_name(typing);
            if self
                .missing_typings_set
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .contains_key(&typing_key)
            {
                logf(
                    logger,
                    format!(
                        "ATA:: '{typing}':: '{typing_key}' is in missingTypingsSet - skipping..."
                    ),
                );
                continue;
            }
            let (validation_result, name, is_scope_name) = validate_package_name(typing);
            if validation_result != NameValidationResult::NameOk {
                self.missing_typings_set
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .insert(typing_key.clone(), true);
                log(
                    logger,
                    &format!(
                        "ATA:: {}",
                        render_package_name_validation_failure(
                            typing,
                            validation_result,
                            &name,
                            is_scope_name
                        )
                    ),
                );
                continue;
            }
            let registry = self
                .types_registry
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            let Some(types_registry_entry) = registry.get(&typing_key) else {
                logf(
                    logger,
                    format!(
                        "ATA:: '{typing}':: Entry for package '{typing_key}' does not exist in local types registry - skipping..."
                    ),
                );
                continue;
            };
            if self
                .package_name_to_typing_location
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .get(&typing_key)
                .is_some_and(|typing_location| {
                    is_typing_up_to_date(typing_location, types_registry_entry)
                })
            {
                logf(
                    logger,
                    format!(
                        "ATA:: '{typing}':: '{typing_key}' already has an up-to-date typing - skipping..."
                    ),
                );
                continue;
            }
            result.push(typing_key);
        }
        result
    }

    fn init(&self, project_id: &str, fs: Arc<dyn vfs::Fs + Send + Sync>, logger: Option<&LogTree>) {
        let mut initialized = self
            .initialized
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        if *initialized {
            return;
        }
        log(
            logger,
            &format!("ATA:: Global cache location '{}'", self.typings_location),
        );
        self.process_cache_location(project_id, fs.clone(), logger);
        self.ensure_typings_location_exists(fs.as_ref(), logger);
        log(
            logger,
            "ATA:: Updating types-registry@latest npm package...",
        );
        match self.npm_executor.npm_install(
            &self.typings_location,
            &[
                "install".to_string(),
                "--ignore-scripts".to_string(),
                "types-registry@latest".to_string(),
            ],
        ) {
            Ok(_) => log(logger, "ATA:: Updated types-registry npm package"),
            Err(err) => logf(
                logger,
                format!("ATA:: Error updating types-registry package: {err}"),
            ),
        }
        *self
            .types_registry
            .lock()
            .unwrap_or_else(|err| err.into_inner()) =
            self.load_types_registry_file(fs.as_ref(), logger);
        *initialized = true;
    }

    fn process_cache_location(
        &self,
        _project_id: &str,
        fs: Arc<dyn vfs::Fs + Send + Sync>,
        logger: Option<&LogTree>,
    ) {
        log(
            logger,
            &format!("ATA:: Processing cache location {}", self.typings_location),
        );
        let package_json = tspath::combine_paths(&self.typings_location, &["package.json"]);
        let package_lock_json =
            tspath::combine_paths(&self.typings_location, &["package-lock.json"]);
        log(logger, &format!("ATA:: Trying to find '{package_json}'..."));
        if fs.file_exists(&package_json) && fs.file_exists(&package_lock_json) {
            let npm_config: NpmConfig = parse_npm_config_or_lock(fs.as_ref(), &package_json);
            let npm_lock: NpmLock = parse_npm_config_or_lock(fs.as_ref(), &package_lock_json);
            let mut resolver = module::new_resolver(
                AtaResolutionHost {
                    current_directory: self.typings_location.clone(),
                    fs: fs.clone(),
                },
                core::CompilerOptions {
                    module_resolution: core::ModuleResolutionKind::NodeNext,
                    ..core::CompilerOptions::default()
                },
                "",
                "",
            );
            for key in npm_config.dev_dependencies.keys() {
                let npm_lock_value = npm_lock
                    .packages
                    .get(&format!("node_modules/{key}"))
                    .or_else(|| npm_lock.dependencies.get(key));
                let Some(npm_lock_value) = npm_lock_value else {
                    continue;
                };
                let package_name = tspath::get_base_file_name(key);
                if package_name.is_empty() {
                    continue;
                }
                let typing_file = self.typing_to_file_name(&mut resolver, &package_name);
                if typing_file.is_empty() {
                    self.missing_typings_set
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .insert(package_name, true);
                    continue;
                }
                if npm_lock_value.version.is_empty() {
                    continue;
                }
                let new_version = semver::must_parse(&npm_lock_value.version);
                self.package_name_to_typing_location
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .insert(
                        package_name.clone(),
                        CachedTyping {
                            typings_location: typing_file,
                            version: new_version,
                        },
                    );
            }
        }
        log(
            logger,
            &format!(
                "ATA:: Finished processing cache location {}",
                self.typings_location
            ),
        );
    }

    fn ensure_typings_location_exists(&self, fs: &dyn vfs::Fs, logger: Option<&LogTree>) {
        let npm_config_path = tspath::combine_paths(&self.typings_location, &["package.json"]);
        log(logger, &format!("ATA:: Npm config file: {npm_config_path}"));
        if !fs.file_exists(&npm_config_path) {
            logf(
                logger,
                format!(
                    "ATA:: Npm config file: '{npm_config_path}' is missing, creating new one..."
                ),
            );
            if let Err(err) = fs.write_file(&npm_config_path, "{ \"private\": true }") {
                logf(logger, format!("ATA:: Npm config file write failed: {err}"));
            }
        }
    }

    fn typing_to_file_name(
        &self,
        resolver: &mut module::Resolver<module::ResolutionHostBox>,
        package_name: &str,
    ) -> String {
        let containing_file = tspath::combine_paths(&self.typings_location, &["index.d.ts"]);
        let (result, _) = resolver.resolve_module_name(
            package_name,
            &containing_file,
            core::ResolutionMode::default(),
            None,
        );
        result.resolved_file_name
    }

    fn load_types_registry_file(
        &self,
        fs: &dyn vfs::Fs,
        logger: Option<&LogTree>,
    ) -> HashMap<String, HashMap<String, String>> {
        let types_registry_file = tspath::combine_paths(
            &self.typings_location,
            &["node_modules/types-registry/index.json"],
        );
        let (contents, ok) = fs.read_file(&types_registry_file);
        if ok {
            let entries: serde_json::Result<TypesRegistryFile> = serde_json::from_str(&contents);
            match entries {
                Ok(entries) => return entries.entries,
                Err(err) => logf(
                    logger,
                    format!(
                        "ATA:: Error when loading types registry file '{types_registry_file}': {err}"
                    ),
                ),
            }
        } else {
            logf(
                logger,
                format!("ATA:: Error reading types registry file '{types_registry_file}'"),
            );
        }
        HashMap::new()
    }
}

const TS_VERSION_TO_USE: &str = "latest";

#[derive(serde::Deserialize, Default)]
struct TypesRegistryFile {
    #[serde(default)]
    entries: HashMap<String, HashMap<String, String>>,
}

#[derive(serde::Deserialize, Default)]
struct NpmConfig {
    #[serde(default, rename = "devDependencies")]
    dev_dependencies: HashMap<String, serde_json::Value>,
}

#[derive(serde::Deserialize, Default)]
struct NpmDependencyEntry {
    #[serde(default)]
    version: String,
}

#[derive(serde::Deserialize, Default)]
struct NpmLock {
    #[serde(default)]
    dependencies: HashMap<String, NpmDependencyEntry>,
    #[serde(default)]
    packages: HashMap<String, NpmDependencyEntry>,
}

fn parse_npm_config_or_lock<T: serde::de::DeserializeOwned + Default>(
    fs: &dyn vfs::Fs,
    location: &str,
) -> T {
    let (contents, _) = fs.read_file(location);
    serde_json::from_str(&contents).unwrap_or_default()
}

fn is_typing_up_to_date(
    cached_typing: &CachedTyping,
    available_typing_versions: &HashMap<String, String>,
) -> bool {
    let use_version = available_typing_versions
        .get(&format!("ts{}", core::version_major_minor()))
        .or_else(|| available_typing_versions.get("latest"))
        .cloned()
        .unwrap_or_default();
    let available_version = semver::must_parse(&use_version);
    available_version.compare(Some(&cached_typing.version)) <= 0
}

fn add_inferred_typing(inferred_typings: &mut HashMap<String, String>, typing_name: &str) {
    inferred_typings.entry(typing_name.to_string()).or_default();
}

fn add_inferred_typings(
    logger: Option<&LogTree>,
    inferred_typings: &mut HashMap<String, String>,
    typing_names: &[String],
    message: &str,
) {
    logf(logger, format!("ATA:: {message}: {typing_names:?}"));
    for typing_name in typing_names {
        add_inferred_typing(inferred_typings, typing_name);
    }
}

fn get_typing_names_from_source_file_names(
    logger: Option<&LogTree>,
    inferred_typings: &mut HashMap<String, String>,
    file_names: &[String],
) {
    let mut has_jsx_file = false;
    let mut from_file_names = Vec::new();
    for file_name in file_names {
        has_jsx_file = has_jsx_file || tspath::file_extension_is(file_name, tspath::EXTENSION_JSX);
        let inferred_typing_name = tspath::remove_file_extension(&tspath::to_file_name_lower_case(
            &tspath::get_base_file_name(file_name),
        ));
        let cleaned_typing_name = remove_min_and_version_numbers(&inferred_typing_name);
        if let Some(type_name) = safe_file_name_to_type_name(&cleaned_typing_name) {
            from_file_names.push(type_name.to_string());
        }
    }
    if !from_file_names.is_empty() {
        add_inferred_typings(
            logger,
            inferred_typings,
            &from_file_names,
            "Inferred typings from file names",
        );
    }
    if has_jsx_file {
        log(
            logger,
            "ATA:: Inferred 'react' typings due to presence of '.jsx' extension",
        );
        add_inferred_typing(inferred_typings, "react");
    }
}

fn add_typing_names_and_get_files_to_watch(
    fs: &dyn vfs::Fs,
    logger: Option<&LogTree>,
    inferred_typings: &mut HashMap<String, String>,
    mut files_to_watch: Vec<String>,
    project_root_path: &str,
    manifest_name: &str,
    modules_dir_name: &str,
) -> Vec<String> {
    let manifest_path = tspath::combine_paths(project_root_path, &[manifest_name]);
    let mut manifest_typing_names = Vec::new();
    let (manifest_contents, ok) = fs.read_file(&manifest_path);
    if ok {
        files_to_watch.push(manifest_path.clone());
        if let Ok(manifest) = packagejson::parse(manifest_contents.as_bytes()) {
            manifest.dependency_fields.range_dependencies(|name, _, _| {
                manifest_typing_names.push(name.to_string());
                true
            });
            add_inferred_typings(
                logger,
                inferred_typings,
                &manifest_typing_names,
                &format!("Typing names in '{manifest_path}' dependencies"),
            );
        }
    }

    let packages_folder_path = tspath::combine_paths(project_root_path, &[modules_dir_name]);
    files_to_watch.push(packages_folder_path.clone());
    if !fs.directory_exists(&packages_folder_path) {
        return files_to_watch;
    }

    let mut dependency_manifest_names = Vec::new();
    if !manifest_typing_names.is_empty() {
        dependency_manifest_names.extend(manifest_typing_names.iter().map(|typing_name| {
            tspath::combine_paths(&packages_folder_path, &[typing_name, manifest_name])
        }));
    } else {
        let entries = fs.get_accessible_entries(&packages_folder_path);
        for directory in entries.directories {
            if directory.starts_with('@') {
                let scope_path = tspath::combine_paths(&packages_folder_path, &[&directory]);
                for scoped in fs.get_accessible_entries(&scope_path).directories {
                    dependency_manifest_names.push(tspath::combine_paths(
                        &scope_path,
                        &[&scoped, manifest_name],
                    ));
                }
            } else {
                dependency_manifest_names.push(tspath::combine_paths(
                    &packages_folder_path,
                    &[&directory, manifest_name],
                ));
            }
        }
    }

    logf(
        logger,
        format!(
            "ATA:: Searching for typing names in {packages_folder_path}; all files: {dependency_manifest_names:?}"
        ),
    );
    for manifest_path in dependency_manifest_names {
        let (manifest_contents, ok) = fs.read_file(&manifest_path);
        if !ok {
            continue;
        }
        let Ok(manifest) = packagejson::parse(manifest_contents.as_bytes()) else {
            continue;
        };
        let package_name = manifest.header_fields.name.value;
        if package_name.is_empty() {
            continue;
        }
        if !manifest.path_fields.types.value.is_empty()
            || !manifest.path_fields.typings.value.is_empty()
        {
            inferred_typings.insert(package_name, manifest_path);
        } else {
            add_inferred_typing(inferred_typings, &package_name);
        }
    }
    files_to_watch
}

fn remove_min_and_version_numbers(file_name: &str) -> String {
    let mut result = file_name.to_string();
    if let Some(stripped) = result.strip_suffix(".min") {
        result = stripped.to_string();
    }
    while let Some(index) = result.rfind(['-', '.']) {
        let suffix = &result[index + 1..];
        if suffix.is_empty() || !suffix.chars().all(|ch| ch.is_ascii_digit()) {
            break;
        }
        result.truncate(index);
    }
    result
}

fn safe_file_name_to_type_name(file_name: &str) -> Option<&'static str> {
    SAFE_FILE_NAME_TO_TYPE_NAME
        .iter()
        .find_map(|(key, value)| (*key == file_name).then_some(*value))
}

static SAFE_FILE_NAME_TO_TYPE_NAME: &[(&str, &str)] = &[
    ("accounting", "accounting"),
    ("ace.js", "ace"),
    ("ag-grid", "ag-grid"),
    ("alertify", "alertify"),
    ("alt", "alt"),
    ("amcharts.js", "amcharts"),
    ("amplify", "amplifyjs"),
    ("angular", "angular"),
    ("angular-bootstrap-lightbox", "angular-bootstrap-lightbox"),
    ("angular-cookie", "angular-cookie"),
    ("angular-file-upload", "angular-file-upload"),
    ("angularfire", "angularfire"),
    ("angular-gettext", "angular-gettext"),
    ("angular-google-analytics", "angular-google-analytics"),
    ("angular-local-storage", "angular-local-storage"),
    ("angularLocalStorage", "angularLocalStorage"),
    ("angular-scroll", "angular-scroll"),
    ("angular-spinner", "angular-spinner"),
    ("angular-strap", "angular-strap"),
    ("angulartics", "angulartics"),
    ("angular-toastr", "angular-toastr"),
    ("angular-translate", "angular-translate"),
    ("angular-ui-router", "angular-ui-router"),
    ("angular-ui-tree", "angular-ui-tree"),
    ("angular-wizard", "angular-wizard"),
    ("async", "async"),
    ("atmosphere", "atmosphere"),
    ("aws-sdk", "aws-sdk"),
    ("aws-sdk-js", "aws-sdk"),
    ("axios", "axios"),
    ("backbone", "backbone"),
    ("backbone.layoutmanager", "backbone.layoutmanager"),
    ("backbone.paginator", "backbone.paginator"),
    ("backbone.radio", "backbone.radio"),
    ("backbone-associations", "backbone-associations"),
    ("backbone-relational", "backbone-relational"),
    ("backgrid", "backgrid"),
    ("Bacon", "baconjs"),
    ("benchmark", "benchmark"),
    ("blazy", "blazy"),
    ("bliss", "blissfuljs"),
    ("bluebird", "bluebird"),
    ("body-parser", "body-parser"),
    ("bootbox", "bootbox"),
    ("bootstrap", "bootstrap"),
    ("bootstrap-editable", "x-editable"),
    ("bootstrap-maxlength", "bootstrap-maxlength"),
    ("bootstrap-notify", "bootstrap-notify"),
    ("bootstrap-slider", "bootstrap-slider"),
    ("bootstrap-switch", "bootstrap-switch"),
    ("bowser", "bowser"),
    ("breeze", "breeze"),
    ("browserify", "browserify"),
    ("bson", "bson"),
    ("c3", "c3"),
    ("canvasjs", "canvasjs"),
    ("chai", "chai"),
    ("chalk", "chalk"),
    ("chance", "chance"),
    ("chartist", "chartist"),
    ("cheerio", "cheerio"),
    ("chokidar", "chokidar"),
    ("chosen.jquery", "chosen"),
    ("chroma", "chroma-js"),
    ("ckeditor.js", "ckeditor"),
    ("cli-color", "cli-color"),
    ("clipboard", "clipboard"),
    ("codemirror", "codemirror"),
    ("colors", "colors"),
    ("commander", "commander"),
    ("commonmark", "commonmark"),
    ("compression", "compression"),
    ("confidence", "confidence"),
    ("connect", "connect"),
    ("Control.FullScreen", "leaflet.fullscreen"),
    ("cookie", "cookie"),
    ("cookie-parser", "cookie-parser"),
    ("cookies", "cookies"),
    ("core", "core-js"),
    ("core-js", "core-js"),
    ("crossfilter", "crossfilter"),
    ("crossroads", "crossroads"),
    ("css", "css"),
    ("ct-ui-router-extras", "ui-router-extras"),
    ("d3", "d3"),
    ("dagre-d3", "dagre-d3"),
    ("dat.gui", "dat-gui"),
    ("debug", "debug"),
    ("deep-diff", "deep-diff"),
    ("Dexie", "dexie"),
    ("dialogs", "angular-dialog-service"),
    ("dojo.js", "dojo"),
    ("doT", "dot"),
    ("dragula", "dragula"),
    ("drop", "drop"),
    ("dropbox", "dropboxjs"),
    ("dropzone", "dropzone"),
    ("Dts Name", "Dts Name"),
    ("dust-core", "dustjs-linkedin"),
    ("easeljs", "easeljs"),
    ("ejs", "ejs"),
    ("ember", "ember"),
    ("envify", "envify"),
    ("epiceditor", "epiceditor"),
    ("es6-promise", "es6-promise"),
    ("ES6-Promise", "es6-promise"),
    ("es6-shim", "es6-shim"),
    ("expect", "expect"),
    ("express", "express"),
    ("express-session", "express-session"),
    ("ext-all.js", "extjs"),
    ("extend", "extend"),
    ("fabric", "fabricjs"),
    ("faker", "faker"),
    ("fastclick", "fastclick"),
    ("favico", "favico.js"),
    ("featherlight", "featherlight"),
    ("FileSaver", "FileSaver"),
    ("fingerprint", "fingerprintjs"),
    ("fixed-data-table", "fixed-data-table"),
    ("flickity.pkgd", "flickity"),
    ("flight", "flight"),
    ("flow", "flowjs"),
    ("Flux", "flux"),
    ("formly", "angular-formly"),
    ("foundation", "foundation"),
    ("fpsmeter", "fpsmeter"),
    ("fuse", "fuse"),
    ("generator", "yeoman-generator"),
    ("gl-matrix", "gl-matrix"),
    ("globalize", "globalize"),
    ("graceful-fs", "graceful-fs"),
    ("gridstack", "gridstack"),
    ("gulp", "gulp"),
    ("gulp-rename", "gulp-rename"),
    ("gulp-uglify", "gulp-uglify"),
    ("gulp-util", "gulp-util"),
    ("hammer", "hammerjs"),
    ("handlebars", "handlebars"),
    ("hasher", "hasher"),
    ("he", "he"),
    ("hello.all", "hellojs"),
    ("highcharts.js", "highcharts"),
    ("highlight", "highlightjs"),
    ("history", "history"),
    ("History", "history"),
    ("hopscotch", "hopscotch"),
    ("hotkeys", "angular-hotkeys"),
    ("html2canvas", "html2canvas"),
    ("humane", "humane"),
    ("i18next", "i18next"),
    ("icheck", "icheck"),
    ("impress", "impress"),
    ("incremental-dom", "incremental-dom"),
    ("Inquirer", "inquirer"),
    ("insight", "insight"),
    ("interact", "interactjs"),
    ("intercom", "intercomjs"),
    ("intro", "intro.js"),
    ("ion.rangeSlider", "ion.rangeSlider"),
    ("ionic", "ionic"),
    ("is", "is_js"),
    ("iscroll", "iscroll"),
    ("jade", "jade"),
    ("jasmine", "jasmine"),
    ("joint", "jointjs"),
    ("jquery", "jquery"),
    ("jquery.address", "jquery.address"),
    ("jquery.are-you-sure", "jquery.are-you-sure"),
    ("jquery.blockUI", "jquery.blockUI"),
    ("jquery.bootstrap.wizard", "jquery.bootstrap.wizard"),
    ("jquery.bootstrap-touchspin", "bootstrap-touchspin"),
    ("jquery.color", "jquery.color"),
    ("jquery.colorbox", "jquery.colorbox"),
    ("jquery.contextMenu", "jquery.contextMenu"),
    ("jquery.cookie", "jquery.cookie"),
    ("jquery.customSelect", "jquery.customSelect"),
    ("jquery.cycle.all", "jquery.cycle"),
    ("jquery.cycle2", "jquery.cycle2"),
    ("jquery.dataTables", "jquery.dataTables"),
    ("jquery.dropotron", "jquery.dropotron"),
    ("jquery.fancybox.pack.js", "fancybox"),
    ("jquery.fancytree-all", "jquery.fancytree"),
    ("jquery.fileupload", "jquery.fileupload"),
    ("jquery.flot", "flot"),
    ("jquery.form", "jquery.form"),
    ("jquery.gridster", "jquery.gridster"),
    ("jquery.handsontable.full", "jquery-handsontable"),
    ("jquery.joyride", "jquery.joyride"),
    ("jquery.jqGrid", "jqgrid"),
    ("jquery.mmenu", "jquery.mmenu"),
    ("jquery.mockjax", "jquery-mockjax"),
    ("jquery.noty", "jquery.noty"),
    ("jquery.payment", "jquery.payment"),
    ("jquery.pjax", "jquery.pjax"),
    ("jquery.placeholder", "jquery.placeholder"),
    ("jquery.qrcode", "jquery.qrcode"),
    ("jquery.qtip", "qtip2"),
    ("jquery.raty", "raty"),
    ("jquery.scrollTo", "jquery.scrollTo"),
    ("jquery.signalR", "signalr"),
    ("jquery.simplemodal", "jquery.simplemodal"),
    ("jquery.timeago", "jquery.timeago"),
    ("jquery.tinyscrollbar", "jquery.tinyscrollbar"),
    ("jquery.tipsy", "jquery.tipsy"),
    ("jquery.tooltipster", "tooltipster"),
    ("jquery.transit", "jquery.transit"),
    ("jquery.uniform", "jquery.uniform"),
    ("jquery.watch", "watch"),
    ("jquery-sortable", "jquery-sortable"),
    ("jquery-ui", "jqueryui"),
    ("js.cookie", "js-cookie"),
    ("js-data", "js-data"),
    ("js-data-angular", "js-data-angular"),
    ("js-data-http", "js-data-http"),
    ("jsdom", "jsdom"),
    ("jsnlog", "jsnlog"),
    ("json5", "json5"),
    ("jspdf", "jspdf"),
    ("jsrender", "jsrender"),
    ("js-signals", "js-signals"),
    ("jstorage", "jstorage"),
    ("jstree", "jstree"),
    ("js-yaml", "js-yaml"),
    ("jszip", "jszip"),
    ("katex", "katex"),
    ("kefir", "kefir"),
    ("keymaster", "keymaster"),
    ("keypress", "keypress"),
    ("kinetic", "kineticjs"),
    ("knockback", "knockback"),
    ("knockout", "knockout"),
    ("knockout.mapping", "knockout.mapping"),
    ("knockout.validation", "knockout.validation"),
    ("knockout-paging", "knockout-paging"),
    ("knockout-pre-rendered", "knockout-pre-rendered"),
    ("ladda", "ladda"),
    ("later", "later"),
    ("lazy", "lazy.js"),
    ("Leaflet.Editable", "leaflet-editable"),
    ("leaflet.js", "leaflet"),
    ("less", "less"),
    ("linq", "linq"),
    ("loading-bar", "angular-loading-bar"),
    ("lodash", "lodash"),
    ("log4javascript", "log4javascript"),
    ("loglevel", "loglevel"),
    ("lokijs", "lokijs"),
    ("lovefield", "lovefield"),
    ("lunr", "lunr"),
    ("lz-string", "lz-string"),
    ("mailcheck", "mailcheck"),
    ("maquette", "maquette"),
    ("marked", "marked"),
    ("math", "mathjs"),
    ("MathJax.js", "mathjax"),
    ("matter", "matter-js"),
    ("md5", "blueimp-md5"),
    ("md5.js", "crypto-js"),
    ("messenger", "messenger"),
    ("method-override", "method-override"),
    ("minimatch", "minimatch"),
    ("minimist", "minimist"),
    ("mithril", "mithril"),
    ("mobile-detect", "mobile-detect"),
    ("mocha", "mocha"),
    ("mock-ajax", "jasmine-ajax"),
    ("modernizr", "modernizr"),
    ("Modernizr", "Modernizr"),
    ("moment", "moment"),
    ("moment-range", "moment-range"),
    ("moment-timezone", "moment-timezone"),
    ("mongoose", "mongoose"),
    ("morgan", "morgan"),
    ("mousetrap", "mousetrap"),
    ("ms", "ms"),
    ("mustache", "mustache"),
    ("native.history", "history"),
    ("nconf", "nconf"),
    ("ncp", "ncp"),
    ("nedb", "nedb"),
    ("ng-cordova", "ng-cordova"),
    ("ngDialog", "ng-dialog"),
    ("ng-flow-standalone", "ng-flow"),
    ("ng-grid", "ng-grid"),
    ("ng-i18next", "ng-i18next"),
    ("ng-table", "ng-table"),
    ("node_redis", "redis"),
    ("node-clone", "clone"),
    ("node-fs-extra", "fs-extra"),
    ("node-glob", "glob"),
    ("Nodemailer", "nodemailer"),
    ("node-mime", "mime"),
    ("node-mkdirp", "mkdirp"),
    ("node-mongodb-native", "mongodb"),
    ("node-mysql", "mysql"),
    ("node-open", "open"),
    ("node-optimist", "optimist"),
    ("node-progress", "progress"),
    ("node-semver", "semver"),
    ("node-tar", "tar"),
    ("node-uuid", "node-uuid"),
    ("node-xml2js", "xml2js"),
    ("nopt", "nopt"),
    ("notify", "notify"),
    ("nouislider", "nouislider"),
    ("npm", "npm"),
    ("nprogress", "nprogress"),
    ("numbro", "numbro"),
    ("numeral", "numeraljs"),
    ("nunjucks", "nunjucks"),
    ("nv.d3", "nvd3"),
    ("object-assign", "object-assign"),
    ("oboe-browser", "oboe"),
    ("office", "office-js"),
    ("offline", "offline-js"),
    ("onsenui", "onsenui"),
    ("OpenLayers.js", "openlayers"),
    ("openpgp", "openpgp"),
    ("p2", "p2"),
    ("packery.pkgd", "packery"),
    ("page", "page"),
    ("pako", "pako"),
    ("papaparse", "papaparse"),
    ("passport", "passport"),
    ("passport-local", "passport-local"),
    ("path", "pathjs"),
    ("pdfkit", "pdfkit"),
    ("peer", "peerjs"),
    ("peg", "pegjs"),
    ("photoswipe", "photoswipe"),
    ("picker.js", "pickadate"),
    ("pikaday", "pikaday"),
    ("pixi", "pixi.js"),
    ("platform", "platform"),
    ("Please", "pleasejs"),
    ("plottable", "plottable"),
    ("polymer", "polymer"),
    ("postal", "postal"),
    ("preloadjs", "preloadjs"),
    ("progress", "progress"),
    ("purify", "dompurify"),
    ("purl", "purl"),
    ("q", "q"),
    ("qs", "qs"),
    ("qunit", "qunit"),
    ("ractive", "ractive"),
    ("rangy-core", "rangy"),
    ("raphael", "raphael"),
    ("raven", "ravenjs"),
    ("react", "react"),
    ("react-bootstrap", "react-bootstrap"),
    ("react-intl", "react-intl"),
    ("react-redux", "react-redux"),
    ("ReactRouter", "react-router"),
    ("ready", "domready"),
    ("redux", "redux"),
    ("request", "request"),
    ("require", "require"),
    ("restangular", "restangular"),
    ("reveal", "reveal"),
    ("rickshaw", "rickshaw"),
    ("rimraf", "rimraf"),
    ("rivets", "rivets"),
    ("rx", "rx"),
    ("rx.angular", "rx-angular"),
    ("sammy", "sammyjs"),
    ("SAT", "sat"),
    ("sax-js", "sax"),
    ("screenfull", "screenfull"),
    ("seedrandom", "seedrandom"),
    ("select2", "select2"),
    ("selectize", "selectize"),
    ("serve-favicon", "serve-favicon"),
    ("serve-static", "serve-static"),
    ("shelljs", "shelljs"),
    ("should", "should"),
    ("showdown", "showdown"),
    ("sigma", "sigmajs"),
    ("signature_pad", "signature_pad"),
    ("sinon", "sinon"),
    ("sjcl", "sjcl"),
    ("slick", "slick-carousel"),
    ("smoothie", "smoothie"),
    ("socket.io", "socket.io"),
    ("socket.io-client", "socket.io-client"),
    ("sockjs", "sockjs-client"),
    ("sortable", "angular-ui-sortable"),
    ("soundjs", "soundjs"),
    ("source-map", "source-map"),
    ("spectrum", "spectrum"),
    ("spin", "spin"),
    ("sprintf", "sprintf"),
    ("stampit", "stampit"),
    ("state-machine", "state-machine"),
    ("Stats", "stats"),
    ("store", "storejs"),
    ("string", "string"),
    ("string_score", "string_score"),
    ("strophe", "strophe"),
    ("stylus", "stylus"),
    ("sugar", "sugar"),
    ("superagent", "superagent"),
    ("svg", "svgjs"),
    ("svg-injector", "svg-injector"),
    ("swfobject", "swfobject"),
    ("swig", "swig"),
    ("swipe", "swipe"),
    ("swiper", "swiper"),
    ("system.js", "systemjs"),
    ("tether", "tether"),
    ("three", "threejs"),
    ("through", "through"),
    ("through2", "through2"),
    ("timeline", "timelinejs"),
    ("tinycolor", "tinycolor"),
    ("tmhDynamicLocale", "angular-dynamic-locale"),
    ("toaster", "angularjs-toaster"),
    ("toastr", "toastr"),
    ("tracking", "tracking"),
    ("trunk8", "trunk8"),
    ("turf", "turf"),
    ("tweenjs", "tweenjs"),
    ("TweenMax", "gsap"),
    ("twig", "twig"),
    ("twix", "twix"),
    ("typeahead.bundle", "typeahead"),
    ("typescript", "typescript"),
    ("ui", "winjs"),
    ("ui-bootstrap-tpls", "angular-ui-bootstrap"),
    ("ui-grid", "ui-grid"),
    ("uikit", "uikit"),
    ("underscore", "underscore"),
    ("underscore.string", "underscore.string"),
    ("update-notifier", "update-notifier"),
    ("url", "jsurl"),
    ("UUID", "uuid"),
    ("validator", "validator"),
    ("vega", "vega"),
    ("vex", "vex-js"),
    ("video", "videojs"),
    ("vue", "vue"),
    ("vue-router", "vue-router"),
    ("webtorrent", "webtorrent"),
    ("when", "when"),
    ("winston", "winston"),
    ("wrench-js", "wrench"),
    ("ws", "ws"),
    ("xlsx", "xlsx"),
    ("xml2json", "x2js"),
    ("xmlbuilder-js", "xmlbuilder"),
    ("xregexp", "xregexp"),
    ("yargs", "yargs"),
    ("yosay", "yosay"),
    ("yui", "yui"),
    ("yui3", "yui"),
    ("zepto", "zepto"),
    ("ZeroClipboard", "zeroclipboard"),
    ("ZSchema-browser", "z-schema"),
];

#[derive(Clone)]
struct AtaResolutionHost {
    current_directory: String,
    fs: Arc<dyn vfs::Fs + Send + Sync>,
}

impl module::ResolutionHost for AtaResolutionHost {
    fn get_current_directory(&self) -> String {
        self.current_directory.clone()
    }

    fn fs(&self) -> &dyn vfs::Fs {
        self.fs.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameValidationResult {
    NameOk,
    EmptyName,
    NameTooLong,
    NameStartsWithDot,
    NameStartsWithUnderscore,
    NameContainsNonUriSafeCharacters,
}

const MAX_PACKAGE_NAME_LENGTH: usize = 214;

pub fn validate_package_name(package_name: &str) -> (NameValidationResult, String, bool) {
    validate_package_name_worker(package_name, true)
}

fn validate_package_name_worker(
    package_name: &str,
    support_scoped_package: bool,
) -> (NameValidationResult, String, bool) {
    if package_name.is_empty() {
        return (NameValidationResult::EmptyName, String::new(), false);
    }
    if package_name.len() > MAX_PACKAGE_NAME_LENGTH {
        return (NameValidationResult::NameTooLong, String::new(), false);
    }
    if package_name.starts_with('.') {
        return (
            NameValidationResult::NameStartsWithDot,
            String::new(),
            false,
        );
    }
    if package_name.starts_with('_') {
        return (
            NameValidationResult::NameStartsWithUnderscore,
            String::new(),
            false,
        );
    }
    if support_scoped_package
        && let Some(without_scope) = package_name.strip_prefix('@')
        && let Some((scope, scoped_package_name)) = without_scope.split_once('/')
        && !scope.is_empty()
        && !scoped_package_name.is_empty()
        && !scoped_package_name.contains('/')
    {
        let (scope_result, _, _) = validate_package_name_worker(scope, false);
        if scope_result != NameValidationResult::NameOk {
            return (scope_result, scope.to_string(), true);
        }
        let (package_result, _, _) = validate_package_name_worker(scoped_package_name, false);
        if package_result != NameValidationResult::NameOk {
            return (package_result, scoped_package_name.to_string(), false);
        }
        return (NameValidationResult::NameOk, String::new(), false);
    }
    if !package_name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~'))
    {
        return (
            NameValidationResult::NameContainsNonUriSafeCharacters,
            String::new(),
            false,
        );
    }
    (NameValidationResult::NameOk, String::new(), false)
}

pub fn render_package_name_validation_failure(
    typing: &str,
    result: NameValidationResult,
    name: &str,
    is_scope_name: bool,
) -> String {
    let kind = if is_scope_name { "Scope" } else { "Package" };
    let name = if name.is_empty() { typing } else { name };
    match result {
        NameValidationResult::EmptyName => {
            format!("'{typing}':: {kind} name '{name}' cannot be empty")
        }
        NameValidationResult::NameTooLong => format!(
            "'{typing}':: {kind} name '{name}' should be less than {MAX_PACKAGE_NAME_LENGTH} characters"
        ),
        NameValidationResult::NameStartsWithDot => {
            format!("'{typing}':: {kind} name '{name}' cannot start with '.'")
        }
        NameValidationResult::NameStartsWithUnderscore => {
            format!("'{typing}':: {kind} name '{name}' cannot start with '_'")
        }
        NameValidationResult::NameContainsNonUriSafeCharacters => {
            format!("'{typing}':: {kind} name '{name}' contains non URI safe characters")
        }
        NameValidationResult::NameOk => panic!("Unexpected Ok result"),
    }
}

fn log(logger: Option<&LogTree>, message: &str) {
    if let Some(logger) = logger {
        logger.log(&[message]);
    }
}

fn logf(logger: Option<&LogTree>, message: String) {
    if let Some(logger) = logger {
        logger.logf(message);
    }
}
