use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use ts_collections as collections;
use ts_core as core;
use ts_tracing as tracing;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use super::{
    CompilerHost, FileLoader, SourceOutputAndProjectReference,
    new_project_reference_dts_faking_host,
};
use crate::projectreferencedtsfakinghost::CompilerHostLike;

struct CompilerHostLikeRef {
    host: Arc<dyn CompilerHost>,
}

impl CompilerHostLikeRef {
    fn new(host: Arc<dyn CompilerHost>) -> Self {
        Self { host }
    }
}

impl CompilerHostLike for CompilerHostLikeRef {
    fn current_directory(&self) -> String {
        self.host.get_current_directory()
    }

    fn use_case_sensitive_file_names(&self) -> bool {
        self.host.fs().use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        self.host.fs().file_exists(path)
    }

    fn directory_exists(&self, path: &str) -> bool {
        self.host.fs().directory_exists(path)
    }

    fn read_file(&self, path: &str) -> Option<String> {
        let (content, ok) = self.host.fs().read_file(path);
        ok.then_some(content)
    }

    fn realpath(&self, path: &str) -> String {
        self.host.fs().realpath(path)
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProjectReferenceParseTask {
    pub config_name: String,
    pub resolved: Option<tsoptions::ParsedCommandLine>,
    pub sub_tasks: Vec<Arc<Mutex<ProjectReferenceParseTask>>>,
}

impl ProjectReferenceParseTask {
    pub fn parse(&mut self, project_reference_parser: &mut ProjectReferenceParser<'_>) {
        let loader = &mut project_reference_parser.loader;
        let trace_done = loader.opts.tracing.as_mut().map(|tracing| {
            tracing.push(
                tracing::Phase::Parse,
                "parseJsonSourceFileConfigFileContent",
                HashMap::from([("path".to_string(), self.config_name.clone())]),
                false,
            )
        });

        self.resolved = loader
            .opts
            .host
            .get_resolved_project_reference(&self.config_name, loader.to_path(&self.config_name));
        if self.resolved.is_none() {
            if let Some(trace_done) = trace_done {
                if let Some(tracing) = loader.opts.tracing.as_mut() {
                    trace_done(tracing);
                }
            }
            return;
        };

        let resolved = self.resolved.as_mut().unwrap();

        resolved.parse_input_output_names();
        let sub_references = resolved.resolved_project_reference_paths();
        if !sub_references.is_empty() {
            self.sub_tasks = create_project_reference_parse_tasks(sub_references);
        }
        if let Some(trace_done) = trace_done {
            if let Some(tracing) = loader.opts.tracing.as_mut() {
                trace_done(tracing);
            }
        }
    }
}

pub fn create_project_reference_parse_tasks(
    project_references: &[String],
) -> Vec<Arc<Mutex<ProjectReferenceParseTask>>> {
    project_references
        .iter()
        .map(|config_name| {
            Arc::new(Mutex::new(ProjectReferenceParseTask {
                config_name: config_name.clone(),
                ..ProjectReferenceParseTask::default()
            }))
        })
        .collect()
}

pub struct ProjectReferenceParser<'a> {
    pub loader: &'a mut FileLoader,
    pub wg: Box<dyn core::WorkGroup>,
    pub tasks_by_file_name:
        collections::SyncMap<tspath::Path, Arc<Mutex<ProjectReferenceParseTask>>>,
}

impl ProjectReferenceParser<'_> {
    pub fn parse(&mut self, mut tasks: Vec<Arc<Mutex<ProjectReferenceParseTask>>>) {
        self.start(&mut tasks);
        self.wg.run_and_wait();
        self.init_mapper(&tasks);
    }

    pub fn start(&mut self, tasks: &mut [Arc<Mutex<ProjectReferenceParseTask>>]) {
        for i in 0..tasks.len() {
            let config_name = tasks[i]
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .config_name
                .clone();
            let path = self.loader.to_path(&config_name);
            let (loaded_task, loaded) = self
                .tasks_by_file_name
                .load_or_store(path, Some(Arc::clone(&tasks[i])));
            if loaded {
                // dedup tasks to ensure correct file order, regardless of which task would be started first
                tasks[i] = loaded_task.unwrap();
            } else {
                let task = Arc::clone(&tasks[i]);
                // PORT NOTE: Go queues project-reference parsing work. Run it
                // inline here so the non-Send AST graph is not moved across a
                // Rust worker boundary while preserving parse order/results.
                let mut sub_tasks = {
                    let mut task = task.lock().unwrap_or_else(|err| err.into_inner());
                    task.parse(self);
                    std::mem::take(&mut task.sub_tasks)
                };
                self.start(&mut sub_tasks);
                task.lock().unwrap_or_else(|err| err.into_inner()).sub_tasks = sub_tasks;
            }
        }
    }

    pub fn init_mapper(&mut self, tasks: &[Arc<Mutex<ProjectReferenceParseTask>>]) {
        let total_references = self.tasks_by_file_name.size() + 1;
        self.loader
            .project_reference_file_mapper
            .config_to_project_reference = HashMap::with_capacity(total_references);
        self.loader
            .project_reference_file_mapper
            .references_in_config_file = HashMap::with_capacity(total_references);
        self.loader
            .project_reference_file_mapper
            .source_to_project_reference = HashMap::new();
        self.loader
            .project_reference_file_mapper
            .output_dts_to_project_reference = HashMap::new();
        let root = self
            .loader
            .opts
            .config
            .config_file
            .as_ref()
            .expect("project references require a root config file")
            .path
            .clone();
        let refs = self.init_mapper_worker(tasks, &mut collections::Set::new());
        self.loader
            .project_reference_file_mapper
            .references_in_config_file
            .insert(root, refs);
        if self
            .loader
            .project_reference_file_mapper
            .opts
            .as_ref()
            .unwrap()
            .can_use_project_reference_source()
            && !self
                .loader
                .project_reference_file_mapper
                .output_dts_to_project_reference
                .is_empty()
        {
            let mapper = self.loader.project_reference_file_mapper.clone();
            let host = new_project_reference_dts_faking_host(
                CompilerHostLikeRef::new(self.loader.opts.host.clone()),
                mapper,
                self.loader
                    .dts_directories
                    .keys()
                    .into_iter()
                    .flatten()
                    .cloned()
                    .collect(),
            );
            self.loader.project_reference_file_mapper.host = Some(Box::new(host.clone()));
            self.loader.project_reference_resolution_host = Some(Box::new(host));
        }
    }

    pub fn init_mapper_worker(
        &mut self,
        tasks: &[Arc<Mutex<ProjectReferenceParseTask>>],
        seen: &mut collections::Set<*const Mutex<ProjectReferenceParseTask>>,
    ) -> Vec<tspath::Path> {
        if tasks.is_empty() {
            return Vec::new();
        }
        let mut results = Vec::with_capacity(tasks.len());
        for task in tasks {
            let task_ptr = Arc::as_ptr(task);
            let (config_name, resolved, sub_tasks) = {
                let task = task.lock().unwrap_or_else(|err| err.into_inner());
                (
                    task.config_name.clone(),
                    task.resolved.clone(),
                    task.sub_tasks.clone(),
                )
            };
            let path = self.loader.to_path(&config_name);
            results.push(path.clone());
            // ensure we only walk each task once
            if !seen.add_if_absent(task_ptr) {
                continue;
            }

            self.loader
                .project_reference_file_mapper
                .config_to_project_reference
                .insert(path.clone(), resolved.clone());
            if let Some(resolved) = &resolved {
                let is_root_config = self
                    .loader
                    .opts
                    .config
                    .config_file
                    .as_ref()
                    .zip(resolved.config_file.as_ref())
                    .is_some_and(|(config_file, resolved_config)| {
                        config_file.path == resolved_config.path
                    });
                if !is_root_config {
                    // Map current task's files first, before recursing into subtasks.
                    // This matches TypeScript's behavior where child project references
                    // overwrite parent entries when a file belongs to multiple projects.
                    self.loader
                        .project_reference_file_mapper
                        .source_to_project_reference
                        .extend(to_project_reference_map(
                            resolved.source_to_project_reference(),
                            resolved,
                        ));
                    self.loader
                        .project_reference_file_mapper
                        .output_dts_to_project_reference
                        .extend(to_project_reference_map(
                            resolved.output_dts_to_project_reference(),
                            resolved,
                        ));
                    if self
                        .loader
                        .project_reference_file_mapper
                        .opts
                        .as_ref()
                        .unwrap()
                        .can_use_project_reference_source()
                    {
                        let compiler_options = resolved.compiler_options();
                        let mut decl_dir = compiler_options.declaration_dir;
                        if decl_dir.is_empty() {
                            decl_dir = compiler_options.out_dir;
                        }
                        if !decl_dir.is_empty() {
                            self.loader
                                .dts_directories
                                .add(self.loader.to_path(&decl_dir));
                        }
                    }
                }
            }

            let references_in_config = self.init_mapper_worker(&sub_tasks, seen);
            self.loader
                .project_reference_file_mapper
                .references_in_config_file
                .insert(path, references_in_config);
        }
        results
    }
}

fn to_project_reference_map(
    references: &std::collections::BTreeMap<
        tspath::Path,
        tsoptions::parsedcommandline::SourceOutputAndProjectReference,
    >,
    resolved: &tsoptions::ParsedCommandLine,
) -> HashMap<tspath::Path, SourceOutputAndProjectReference> {
    references
        .iter()
        .map(|(path, reference)| {
            (
                path.clone(),
                SourceOutputAndProjectReference {
                    source: reference.source.clone(),
                    output_dts: reference.output_dts.clone(),
                    resolved: Some(Box::new(resolved.clone())),
                },
            )
        })
        .collect()
}
