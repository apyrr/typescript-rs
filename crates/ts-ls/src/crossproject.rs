use std::backtrace::Backtrace;
use std::collections::HashMap;
use std::panic;
use std::sync::Arc;

use ts_collections as collections;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto::{self as lsproto, DocumentUriExt, HasLocation};
use ts_tspath as tspath;

use crate::LanguageService;
use crate::findallreferences::{SymbolAndEntriesData, SymbolEntryTransformOptions};

pub trait Project {
    fn id(&self) -> tspath::Path;
    fn get_program(&self) -> Option<&compiler::Program>;
    fn has_file(&self, file_name: &str) -> bool;
}

struct ProjectAndTextDocumentPosition<'a> {
    project: Arc<dyn Project>,
    ls: Option<LanguageServiceSlot<'a>>,
    uri: lsproto::DocumentUri,
    position: lsproto::Position,
    for_original_location: bool,
}

enum LanguageServiceSlot<'a> {
    Borrowed(&'a LanguageService<'a>),
    Owned(LanguageService<'static>),
}

impl LanguageServiceSlot<'_> {
    fn as_ref(&self) -> &LanguageService<'_> {
        match self {
            Self::Borrowed(ls) => ls,
            Self::Owned(ls) => ls,
        }
    }
}

#[derive(Clone)]
struct Response<Resp> {
    complete: bool,
    result: Resp,
    for_original_location: bool,
}

pub trait CrossProjectOrchestrator {
    fn get_default_project(&self) -> Arc<dyn Project>;
    fn get_all_projects_for_initial_request(&self) -> Vec<Arc<dyn Project>>;
    fn get_language_service_for_project_with_file(
        &self,
        ctx: &core::Context,
        project: &dyn Project,
        uri: lsproto::DocumentUri,
    ) -> Option<LanguageService<'static>>;
    fn get_projects_for_file(
        &self,
        ctx: &core::Context,
        uri: lsproto::DocumentUri,
    ) -> Result<Vec<Arc<dyn Project>>, core::Error>;
    fn get_projects_loading_project_tree(
        &self,
        ctx: &core::Context,
        requested_project_trees: &collections::Set<tspath::Path>,
    ) -> Box<dyn Iterator<Item = Arc<dyn Project>> + '_>;
}

pub(crate) fn handle_cross_project<Req, Resp, F, G>(
    default_ls: &LanguageService<'_>,
    ctx: &core::Context,
    params: Req,
    orchestrator: Option<&dyn CrossProjectOrchestrator>,
    symbol_and_entries_to_resp: F,
    combine_results: G,
    is_rename: bool,
    implementations: bool,
    options: SymbolEntryTransformOptions,
) -> Result<Resp, core::Error>
where
    Req: lsproto::HasTextDocumentPosition + Clone,
    Resp: Default + Clone + 'static,
    F: Fn(
        &LanguageService<'_>,
        &core::Context,
        Req,
        SymbolAndEntriesData,
        SymbolEntryTransformOptions,
    ) -> Result<Resp, core::Error>,
    G: Fn(Box<dyn Iterator<Item = Resp>>) -> Resp,
{
    let mut resp = Resp::default();
    let mut err = None;

    // Single project
    let Some(orchestrator) = orchestrator else {
        let (data, _) = default_ls.provide_symbols_and_entries(
            ctx,
            params.text_document_uri(),
            params.text_document_position(),
            is_rename,
            implementations,
        )?;
        return symbol_and_entries_to_resp(default_ls, ctx, params, data, options);
    };

    let default_project = orchestrator.get_default_project();
    let all_projects = orchestrator.get_all_projects_for_initial_request();
    let results: collections::SyncMap<tspath::Path, Response<Resp>> = collections::SyncMap::new();
    let mut default_definition = None;
    let can_search_project =
        |project: &dyn Project, results: &collections::SyncMap<tspath::Path, Response<Resp>>| {
            let (_, searched) = results.load(&project.id());
            !searched
        };
    let mut queue = Vec::new();
    let mut panics_occurred = Vec::new();

    fn enqueue_item<'a, Resp>(
        results: &collections::SyncMap<tspath::Path, Response<Resp>>,
        queue: &mut Vec<ProjectAndTextDocumentPosition<'a>>,
        item: ProjectAndTextDocumentPosition<'a>,
    ) where
        Resp: Default + Clone,
    {
        let response = Response {
            complete: false,
            result: Resp::default(),
            for_original_location: false,
        };
        if results.load_or_store(item.project.id(), Some(response)).1 {
            return;
        }
        queue.push(item);
    }

    // Initial set of projects and locations in the queue, starting with default project
    enqueue_item(
        &results,
        &mut queue,
        ProjectAndTextDocumentPosition {
            project: default_project.clone(),
            ls: Some(LanguageServiceSlot::Borrowed(default_ls)),
            uri: params.text_document_uri(),
            position: params.text_document_position(),
            for_original_location: false,
        },
    );
    for project in &all_projects {
        if project.id() != default_project.id() {
            enqueue_item(
                &results,
                &mut queue,
                ProjectAndTextDocumentPosition {
                    project: project.clone(),
                    ls: None,
                    // Symlinks need to change the URI (matches Go note).
                    uri: params.text_document_uri(),
                    position: params.text_document_position(),
                    for_original_location: false,
                },
            );
        }
    }

    let get_results_iterator = || {
        let mut values = Vec::new();
        let seen_projects = collections::SyncSet::new();
        if let (Some(response), true) = results.load(&default_project.id()) {
            if response.complete {
                values.push(response.result.clone());
            }
        }
        seen_projects.add(default_project.id());
        for project in &all_projects {
            if seen_projects.add_if_absent(project.id()) {
                if let (Some(response), true) = results.load(&project.id()) {
                    if response.complete {
                        values.push(response.result.clone());
                    }
                }
            }
        }
        // Prefer the searches from locations for default definition
        results.range(|key, response| {
            let Some(response) = response else {
                return true;
            };
            if !response.for_original_location
                && seen_projects.add_if_absent(key.clone())
                && response.complete
            {
                values.push(response.result.clone());
            }
            true
        });
        // Then the searches from original locations
        results.range(|key, response| {
            let Some(response) = response else {
                return true;
            };
            if response.for_original_location
                && seen_projects.add_if_absent(key.clone())
                && response.complete
            {
                values.push(response.result.clone());
            }
            true
        });
        values.into_iter()
    };

    // Outer loop - to complete work if more is added after completing existing queue
    loop {
        // Process existing known projects first
        while let Some(item) = queue.pop() {
            let panic_result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                if ctx.err().is_some() {
                    return;
                }
                // Process the item
                let ls = match item.ls {
                    Some(ls) => ls,
                    None => {
                        let Some(ls) = orchestrator.get_language_service_for_project_with_file(
                            ctx,
                            item.project.as_ref(),
                            item.uri.clone(),
                        ) else {
                            return;
                        };
                        LanguageServiceSlot::Owned(ls)
                    }
                };
                let ls = ls.as_ref();
                let (data, ok) = match ls.provide_symbols_and_entries(
                    ctx,
                    item.uri.clone(),
                    item.position,
                    is_rename,
                    implementations,
                ) {
                    Ok(result) => result,
                    Err(err_search) => {
                        if err.is_none() {
                            err = Some(err_search);
                        }
                        return;
                    }
                };
                if ctx.err().is_some() {
                    return;
                }
                if ok {
                    for entry in &data.symbols_and_entries {
                        // Find the default definition that can be in another project
                        // Later we will use this load ancestor tree that references this location and expand search
                        if item.project.id() == default_project.id() && default_definition.is_none()
                        {
                            match ls.get_non_local_definition(ctx, entry) {
                                Ok(definition) => default_definition = definition,
                                Err(err_search) => {
                                    if err.is_none() {
                                        err = Some(err_search);
                                    }
                                    return;
                                }
                            }
                        }
                        ls.for_each_original_definition_location(ctx, entry, |uri, position| {
                            // Get default configured project for this file
                            let def_projects = orchestrator.get_projects_for_file(ctx, uri.clone());
                            let Ok(def_projects) = def_projects else {
                                return;
                            };
                            for def_project in def_projects {
                                // Optimization: don't enqueue if will be discarded
                                if can_search_project(def_project.as_ref(), &results) {
                                    enqueue_item(
                                        &results,
                                        &mut queue,
                                        ProjectAndTextDocumentPosition {
                                            project: def_project.clone(),
                                            ls: None,
                                            uri: uri.clone(),
                                            position,
                                            for_original_location: true,
                                        },
                                    );
                                }
                            }
                        });
                    }
                }

                match symbol_and_entries_to_resp(ls, ctx, params.clone(), data, options) {
                    Ok(result) => {
                        results.store(
                            item.project.id(),
                            Some(Response {
                                complete: true,
                                result,
                                for_original_location: item.for_original_location,
                            }),
                        );
                    }
                    Err(err_search) => {
                        if err.is_none() {
                            err = Some(err_search);
                        }
                    }
                }
            }));
            if let Err(r) = panic_result {
                let panic_occurred = format!(
                    "panic handling request: {:?}\n{}",
                    r,
                    Backtrace::force_capture()
                );
                panics_occurred.push(panic_occurred);
            }
        }
        // No need to use mu here since we are not in parallel at this point
        if !panics_occurred.is_empty() {
            panic!(
                "Panics occurred during cross-project handling: {:?}",
                panics_occurred
            );
        }
        if let Some(ctx_err) = ctx.err() {
            return Err(core::Error::new(ctx_err));
        }
        if let Some(err) = err.take() {
            return Err(err);
        }

        let mut has_more_work = false;
        if let Some(default_definition) = default_definition.as_ref() {
            let mut requested_project_trees = collections::Set::new();
            results.range(|key, response| {
                if response.is_some_and(|response| response.complete) {
                    requested_project_trees.add(key.clone());
                }
                true
            });

            // Load more projects based on default definition found
            for loaded_project in
                orchestrator.get_projects_loading_project_tree(ctx, &requested_project_trees)
            {
                if let Some(ctx_err) = ctx.err() {
                    return Err(core::Error::new(ctx_err));
                }

                // Can loop forever without this (enqueue here, dequeue above, repeat)
                if !can_search_project(loaded_project.as_ref(), &results)
                    || loaded_project.get_program().is_none()
                {
                    continue;
                }

                // Enqueue the project and location for further processing
                if loaded_project.has_file(&default_definition.position.uri.file_name()) {
                    enqueue_item(
                        &results,
                        &mut queue,
                        ProjectAndTextDocumentPosition {
                            project: loaded_project.clone(),
                            ls: None,
                            uri: default_definition.position.uri.clone(),
                            position: default_definition.position.pos,
                            for_original_location: false,
                        },
                    );
                    has_more_work = true;
                } else if let Some(source_pos) = (default_definition.get_source_position)()
                    .filter(|source_pos| loaded_project.has_file(&source_pos.uri.file_name()))
                {
                    enqueue_item(
                        &results,
                        &mut queue,
                        ProjectAndTextDocumentPosition {
                            project: loaded_project.clone(),
                            ls: None,
                            uri: source_pos.uri.clone(),
                            position: source_pos.pos,
                            for_original_location: false,
                        },
                    );
                    has_more_work = true;
                } else if let Some(generated_pos) = (default_definition.get_generated_position)()
                    .filter(|generated_pos| loaded_project.has_file(&generated_pos.uri.file_name()))
                {
                    enqueue_item(
                        &results,
                        &mut queue,
                        ProjectAndTextDocumentPosition {
                            project: loaded_project.clone(),
                            ls: None,
                            uri: generated_pos.uri.clone(),
                            position: generated_pos.pos,
                            for_original_location: false,
                        },
                    );
                    has_more_work = true;
                }
            }
        }
        if !has_more_work {
            break;
        }
    }

    if results.size() > 1 {
        resp = combine_results(Box::new(get_results_iterator()));
    } else {
        // Single result, return that directly
        for value in get_results_iterator() {
            resp = value;
            break;
        }
    }
    Ok(resp)
}

pub(crate) fn combine_location_array<T>(
    mut combined: Vec<T>,
    locations: &[T],
    seen: &mut collections::Set<lsproto::Location>,
) -> Vec<T>
where
    T: lsproto::HasLocation + Clone,
{
    for loc in locations {
        if seen.add_if_absent(loc.get_location()) {
            combined.push(loc.clone());
        }
    }
    combined
}

pub(crate) fn combine_response_locations(
    results: impl Iterator<Item = lsproto::ReferencesResponse>,
) -> Vec<lsproto::Location> {
    let mut combined = Vec::new();
    let mut seen_locations = collections::Set::new();
    for resp in results {
        if let Some(locations) = resp.locations {
            combined = combine_location_array(combined, &locations, &mut seen_locations);
        }
    }
    combined
}

pub(crate) fn combine_references(
    results: impl Iterator<Item = lsproto::ReferencesResponse>,
) -> lsproto::ReferencesResponse {
    lsproto::LocationsOrNull {
        locations: Some(combine_response_locations(results)),
        ..Default::default()
    }
}

pub(crate) fn combine_implementations(
    mut results: impl Iterator<Item = lsproto::ImplementationResponse>,
) -> lsproto::ImplementationResponse {
    let mut combined = Vec::new();
    let mut seen_locations = collections::Set::new();
    while let Some(resp) = results.next() {
        if let Some(definition_links) = resp.definition_links {
            let definition_links: Vec<_> = definition_links.into_iter().flatten().collect();
            combined = combine_location_array(combined, &definition_links, &mut seen_locations);
        } else if resp.locations.is_some() {
            let remaining = std::iter::once(resp)
                .chain(results)
                .filter_map(|resp| resp.locations)
                .flat_map(|locations| locations.into_iter());
            let mut locations = Vec::new();
            for location in remaining {
                if seen_locations.add_if_absent(location.clone()) {
                    locations.push(location);
                }
            }
            return lsproto::LocationOrLocationsOrDefinitionLinksOrNull {
                locations: Some(locations),
                ..Default::default()
            };
        }
    }
    lsproto::LocationOrLocationsOrDefinitionLinksOrNull {
        definition_links: Some(combined.into_iter().map(Some).collect()),
        ..Default::default()
    }
}

pub(crate) fn combine_rename_response(
    results: impl Iterator<Item = lsproto::RenameResponse>,
) -> lsproto::RenameResponse {
    let mut combined: HashMap<lsproto::DocumentUri, Vec<lsproto::TextEdit>> = HashMap::new();
    let mut seen_changes: HashMap<lsproto::DocumentUri, collections::Set<lsproto::Range>> =
        HashMap::new();
    let mut document_changes = Vec::new();
    let mut seen_renames = collections::Set::new();

    for resp in results {
        if let Some(workspace_edit) = resp.workspace_edit {
            if let Some(changes) = workspace_edit.document_changes {
                for change in changes {
                    if let Some(rename_file) = &change.rename_file {
                        let key = (rename_file.old_uri.clone(), rename_file.new_uri.clone());
                        if seen_renames.add_if_absent(key) {
                            document_changes.push(change);
                        }
                    } else {
                        document_changes.push(change);
                    }
                }
            }
            if let Some(changes) = workspace_edit.changes {
                for (doc, changes) in changes {
                    let seen_set = seen_changes
                        .entry(doc.clone())
                        .or_insert_with(collections::Set::new);
                    let changes_for_doc = combined.entry(doc.clone()).or_default();
                    for change in changes {
                        if !seen_set.has(&change.range) {
                            seen_set.add(change.range.clone());
                            changes_for_doc.push(change);
                        }
                    }
                }
            }
        }
    }
    if !document_changes.is_empty() || !combined.is_empty() {
        let mut workspace_edit = lsproto::WorkspaceEdit::default();
        if !document_changes.is_empty() {
            workspace_edit.document_changes = Some(document_changes);
        }
        if !combined.is_empty() {
            workspace_edit.changes = Some(combined);
        }
        return lsproto::RenameResponse {
            workspace_edit: Some(workspace_edit),
            ..Default::default()
        };
    }
    lsproto::RenameResponse::default()
}

pub(crate) fn combine_incoming_calls(
    results: impl Iterator<Item = lsproto::CallHierarchyIncomingCallsResponse>,
) -> lsproto::CallHierarchyIncomingCallsResponse {
    let mut combined = Vec::new();
    let mut seen_calls = collections::Set::new();
    for resp in results {
        if let Some(call_hierarchy_incoming_calls) = resp.call_hierarchy_incoming_calls {
            for call in call_hierarchy_incoming_calls.into_iter().flatten() {
                if seen_calls.add_if_absent(call.from.get_location()) {
                    combined.push(call);
                }
            }
        }
    }
    lsproto::CallHierarchyIncomingCallsResponse {
        call_hierarchy_incoming_calls: Some(combined.into_iter().map(Some).collect()),
        ..Default::default()
    }
}
