use ts_collections as collections;
use ts_core as context;

use crate::{
    ApiSnapshotRequest, FileChangeSummary, Project, Session, SnapshotChange, SnapshotHandle,
    merge_file_change_summary,
};

// APIOpenProject opens a project and returns a ref'd snapshot.
// The caller must call snapshot.Deref(s) when done.
impl Session {
    pub fn api_open_project(
        &mut self,
        ctx: &context::Context,
        config_file_name: String,
        api_file_changes: FileChangeSummary,
    ) -> (
        Option<Project>,
        SnapshotHandle,
        Option<Box<dyn std::error::Error + Send + Sync>>,
    ) {
        let (mut file_changes, overlays, ata_changes, _) = self.flush_changes(ctx.clone());
        merge_file_change_summary(&mut file_changes, api_file_changes);
        self.update_snapshot_ref(
            ctx.clone(),
            overlays,
            SnapshotChange {
                file_changes,
                ata_changes,
                api_request: Some(ApiSnapshotRequest {
                    open_projects: Some(collections::new_set_from_items(
                        [config_file_name.clone()],
                    )),
                    close_projects: None,
                }),
                ..Default::default()
            },
        );
        let new_snapshot = self.snapshot.clone_handle();

        if new_snapshot.snapshot().api_error.is_some() {
            let api_error = new_snapshot
                .snapshot()
                .api_error
                .as_ref()
                .unwrap()
                .to_string();
            return (None, new_snapshot, Some(api_error.into()));
        }

        let project = new_snapshot
            .snapshot()
            .project_collection
            .configured_project(self.to_path(&config_file_name));
        if project.is_none() {
            panic!("OpenProject request returned no error but project not present in snapshot");
        }

        (project.cloned(), new_snapshot, None)
    }

    // APIUpdateWithFileChanges creates a new snapshot incorporating the given
    // file changes. Returns a ref'd snapshot; caller must Deref when done.
    pub fn api_update_with_file_changes(
        &mut self,
        ctx: &context::Context,
        api_file_changes: FileChangeSummary,
    ) -> SnapshotHandle {
        let (mut file_changes, overlays, ata_changes, _) = self.flush_changes(ctx.clone());
        merge_file_change_summary(&mut file_changes, api_file_changes);

        self.update_snapshot_ref(
            ctx.clone(),
            overlays,
            SnapshotChange {
                api_request: Some(ApiSnapshotRequest {
                    open_projects: None,
                    close_projects: None,
                }),
                file_changes,
                ata_changes,
                ..Default::default()
            },
        );
        self.snapshot.clone_handle()
    }
}
