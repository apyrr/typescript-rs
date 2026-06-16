use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

use ts_ast as ast;
use ts_checker as checker;
use ts_core as core;

use crate::{Program, sort_and_deduplicate_diagnostics};

pub type CheckerCallback<'program, 'callback> =
    dyn for<'checker, 'state> FnMut(ActiveChecker<'program, 'checker, 'state>) + 'callback;

pub fn checker_slot_index_from_state_identity(identity: checker::CheckerStateIdentity) -> usize {
    let slot_id = identity.slot().get();
    let slot_index = slot_id
        .checked_sub(1)
        .expect("checker state identity slot id must be one-based");
    usize::try_from(slot_index).expect("checker slot index must fit usize")
}

pub enum CheckerAccess<'program, 'access, 'checker, 'state> {
    Context(&'access core::Context),
    Active(&'access mut ActiveChecker<'program, 'checker, 'state>),
}

impl<'program, 'access, 'checker, 'state> CheckerAccess<'program, 'access, 'checker, 'state> {
    pub fn context(ctx: &'access core::Context) -> Self {
        Self::Context(ctx)
    }

    pub fn active(active: &'access mut ActiveChecker<'program, 'checker, 'state>) -> Self {
        Self::Active(active)
    }
}

pub struct ActiveChecker<'program, 'checker, 'state> {
    program: &'program Program,
    checker: &'checker mut checker::Checker<'program, 'state>,
}

impl<'program, 'checker, 'state> ActiveChecker<'program, 'checker, 'state> {
    pub fn new(
        program: &'program Program,
        checker: &'checker mut checker::Checker<'program, 'state>,
    ) -> Self {
        Self { program, checker }
    }

    pub fn program(&self) -> &'program Program {
        self.program
    }

    pub fn checker(&mut self) -> &mut checker::Checker<'program, 'state> {
        self.checker
    }

    pub fn state_identity(&self) -> checker::CheckerStateIdentity {
        self.checker.state_identity()
    }
}

impl<'program, 'checker, 'state> Deref for ActiveChecker<'program, 'checker, 'state> {
    type Target = checker::Checker<'program, 'state>;

    fn deref(&self) -> &Self::Target {
        self.checker
    }
}

impl<'program, 'checker, 'state> DerefMut for ActiveChecker<'program, 'checker, 'state> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.checker
    }
}

impl<'program, 'checker, 'state> ts_modulespecifiers::CheckerShape
    for ActiveChecker<'program, 'checker, 'state>
{
    fn source_file_store(&self, node: ast::Node) -> Option<&ast::AstStore> {
        ts_modulespecifiers::CheckerShape::source_file_store(&*self.checker, node)
    }

    fn source_node_symbol(&self, node: ast::Node) -> Option<ast::SymbolIdentity> {
        ts_modulespecifiers::CheckerShape::source_node_symbol(&*self.checker, node)
    }

    fn lookup_source_symbol_export(
        &mut self,
        symbol: ast::SymbolIdentity,
        name: &str,
    ) -> Option<ast::SymbolIdentity> {
        ts_modulespecifiers::CheckerShape::lookup_source_symbol_export(
            &mut *self.checker,
            symbol,
            name,
        )
    }

    fn symbol_value_declaration(&self, symbol: ast::SymbolIdentity) -> Option<ast::Node> {
        ts_modulespecifiers::CheckerShape::symbol_value_declaration(&*self.checker, symbol)
    }

    fn get_symbol_at_location(
        &mut self,
        node: ast::Node,
    ) -> Option<ts_modulespecifiers::SpecifierSymbol> {
        ts_modulespecifiers::CheckerShape::get_symbol_at_location(&mut *self.checker, node)
    }

    fn get_aliased_symbol_at_location(
        &mut self,
        node: ast::Node,
    ) -> Option<ts_modulespecifiers::SpecifierSymbol> {
        ts_modulespecifiers::CheckerShape::get_aliased_symbol_at_location(&mut *self.checker, node)
    }
}

type SharedCheckerState = Arc<Mutex<checker::CheckerState>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActivePoolSlot {
    pool: usize,
    slot: usize,
}

thread_local! {
    static ACTIVE_POOL_SLOTS: RefCell<Vec<ActivePoolSlot>> = const { RefCell::new(Vec::new()) };
}

struct ActivePoolSlotGuard {
    slot: ActivePoolSlot,
}

impl ActivePoolSlotGuard {
    fn new(pool: usize, slot: usize) -> Self {
        let slot = ActivePoolSlot { pool, slot };
        ACTIVE_POOL_SLOTS.with_borrow_mut(|slots| slots.push(slot));
        Self { slot }
    }
}

impl Drop for ActivePoolSlotGuard {
    fn drop(&mut self) {
        ACTIVE_POOL_SLOTS.with_borrow_mut(|slots| {
            let active = slots
                .pop()
                .expect("active checker stack must contain the current checker slot");
            if active != self.slot {
                panic!("active checker stack corrupted by out-of-order checker drop");
            }
        });
    }
}

fn active_slot_for_current_thread(pool: usize, slot: usize) -> bool {
    ACTIVE_POOL_SLOTS.with_borrow(|slots| {
        slots
            .iter()
            .any(|active| active.pool == pool && active.slot == slot)
    })
}

fn assert_not_reentrant(pool: usize, slot: usize) {
    if active_slot_for_current_thread(pool, slot) {
        panic!(
            "nested checker acquisition for an active checker slot requires CheckerAccess::Active"
        );
    }
}

// CheckerPool is implemented by the project system to provide request-scoped
// checker access. The callback runs while the checker slot is held by the pool,
// so the mutable checker view cannot escape.
// If `file` is `Some`, the pool returns the checker associated with that file;
// otherwise it returns the first checker. Types and checker-local handles
// obtained from different checkers must not be compared.
pub trait CheckerPool: Send + Sync {
    fn with_checker<'program>(
        &'program self,
        program: &'program Program,
        ctx: &core::Context,
        file: Option<&'program ast::SourceFile>,
        cb: &mut CheckerCallback<'program, '_>,
    );

    fn with_checker_for_state_identity<'program>(
        &'program self,
        program: &'program Program,
        ctx: &core::Context,
        identity: checker::CheckerStateIdentity,
        cb: &mut CheckerCallback<'program, '_>,
    );
}

pub struct NullCheckerPool;

impl CheckerPool for NullCheckerPool {
    fn with_checker<'program>(
        &'program self,
        _program: &'program Program,
        _ctx: &core::Context,
        _file: Option<&'program ast::SourceFile>,
        _cb: &mut CheckerCallback<'program, '_>,
    ) {
        panic!("NullCheckerPool should be replaced by init_checker_pool before use")
    }

    fn with_checker_for_state_identity<'program>(
        &'program self,
        _program: &'program Program,
        _ctx: &core::Context,
        _identity: checker::CheckerStateIdentity,
        _cb: &mut CheckerCallback<'program, '_>,
    ) {
        panic!("NullCheckerPool should be replaced by init_checker_pool before use")
    }
}

struct CheckerSlot {
    semantic_state: SharedCheckerState,
    slot_lock: Mutex<()>,
}

impl CheckerSlot {
    fn new(slot_index: usize) -> Self {
        Self {
            semantic_state: Arc::new(Mutex::new(checker::CheckerState::new_for_slot_index(
                slot_index,
            ))),
            slot_lock: Mutex::new(()),
        }
    }
}

pub struct CheckerPoolImpl {
    slots: Vec<CheckerSlot>,
    file_associations: HashMap<checker::SourceFileIdentity, usize>,
}

pub(crate) fn new_checker_pool(program: &Program) -> CheckerPoolImpl {
    let mut checker_count = 4;
    if program.single_threaded() {
        checker_count = 1;
    } else if let Some(c) = program.options().checkers {
        checker_count = c;
    }

    checker_count = checker_count
        .min(program.source_files.len())
        .min(256)
        .max(1);

    let slots = (0..checker_count).map(CheckerSlot::new).collect();
    let file_associations = program
        .source_files
        .iter()
        .enumerate()
        .map(|(file_index, file)| {
            (
                checker::SourceFileIdentity::from_source_file(file.as_source_file()),
                file_index % checker_count,
            )
        })
        .collect();

    CheckerPoolImpl {
        slots,
        file_associations,
    }
}

impl CheckerPool for CheckerPoolImpl {
    fn with_checker<'program>(
        &'program self,
        program: &'program Program,
        ctx: &core::Context,
        file: Option<&'program ast::SourceFile>,
        cb: &mut CheckerCallback<'program, '_>,
    ) {
        let _ = ctx;
        let slot_index = file
            .map(|file| self.checker_index_for_file(program, file))
            .unwrap_or(0);
        self.with_slot_exclusive(program, slot_index, cb);
    }

    fn with_checker_for_state_identity<'program>(
        &'program self,
        program: &'program Program,
        ctx: &core::Context,
        identity: checker::CheckerStateIdentity,
        cb: &mut CheckerCallback<'program, '_>,
    ) {
        let _ = ctx;
        self.with_slot_exclusive(
            program,
            checker_slot_index_from_state_identity(identity),
            cb,
        );
    }
}

impl CheckerPoolImpl {
    fn pool_id(&self) -> usize {
        self as *const Self as usize
    }

    fn checker_count(&self) -> usize {
        self.slots.len()
    }

    fn checker_index_for_file(&self, _program: &Program, file: &ast::SourceFile) -> usize {
        self.file_associations
            .get(&checker::SourceFileIdentity::from_source_file(file))
            .copied()
            .expect("checker pool file affinity requires a program source file")
    }

    pub(crate) fn file_matches_state_identity(
        &self,
        program: &Program,
        file: &ast::SourceFile,
        identity: checker::CheckerStateIdentity,
    ) -> bool {
        self.checker_index_for_file(program, file)
            == checker_slot_index_from_state_identity(identity)
    }

    fn with_slot_exclusive<'program>(
        &'program self,
        program: &'program Program,
        slot_index: usize,
        cb: &mut CheckerCallback<'program, '_>,
    ) {
        assert_not_reentrant(self.pool_id(), slot_index);
        let slot = self
            .slots
            .get(slot_index)
            .expect("checker slot index must be in bounds");
        let _slot_guard = slot.slot_lock.lock().unwrap_or_else(|err| err.into_inner());
        with_shared_state(
            program,
            self.pool_id(),
            slot_index,
            &slot.semantic_state,
            true,
            cb,
        );
    }

    pub(crate) fn with_checker_for_file_non_exclusive<'program>(
        &'program self,
        program: &'program Program,
        file: &'program ast::SourceFile,
        cb: &mut CheckerCallback<'program, '_>,
    ) {
        let slot_index = self.checker_index_for_file(program, file);
        let slot = self
            .slots
            .get(slot_index)
            .expect("checker slot index must be in bounds");
        with_shared_state(
            program,
            self.pool_id(),
            slot_index,
            &slot.semantic_state,
            false,
            cb,
        );
    }

    pub(crate) fn with_checker_for_state_identity_non_exclusive<'program>(
        &'program self,
        program: &'program Program,
        identity: checker::CheckerStateIdentity,
        cb: &mut CheckerCallback<'program, '_>,
    ) {
        let slot_index = checker_slot_index_from_state_identity(identity);
        let slot = self
            .slots
            .get(slot_index)
            .expect("checker slot index must be in bounds");
        with_shared_state(
            program,
            self.pool_id(),
            slot_index,
            &slot.semantic_state,
            false,
            cb,
        );
    }

    pub(crate) fn with_checker_for_file_exclusive<'program>(
        &'program self,
        ctx: &core::Context,
        program: &'program Program,
        file: &'program ast::SourceFile,
        cb: &mut CheckerCallback<'program, '_>,
    ) {
        let _ = ctx;
        self.with_slot_exclusive(program, self.checker_index_for_file(program, file), cb);
    }

    // Runs `cb` for each checker in the pool concurrently, locking and unlocking
    // checker mutexes as it goes, making it safe to call `forEachCheckerParallel`
    // from many threads simultaneously.
    pub(crate) fn for_each_checker_parallel<'program, F>(
        &'program self,
        program: &'program Program,
        cb: F,
    ) where
        F: for<'checker, 'state> Fn(usize, &'checker mut checker::Checker<'program, 'state>) + Sync,
    {
        let checker_count = self.checker_count();
        if program.single_threaded() || checker_count == 1 {
            self.for_each_checker_parallel_serial(program, checker_count, &cb);
            return;
        }

        std::thread::scope(|scope| {
            for idx in 0..checker_count {
                let cb = &cb;
                scope.spawn(move || {
                    let mut slot_cb = |mut active: ActiveChecker<'program, '_, '_>| {
                        cb(idx, active.checker());
                    };
                    self.with_slot_exclusive(program, idx, &mut slot_cb);
                });
            }
        });
    }

    fn for_each_checker_parallel_serial<'program, F>(
        &'program self,
        program: &'program Program,
        checker_count: usize,
        cb: &F,
    ) where
        F: for<'checker, 'state> Fn(usize, &'checker mut checker::Checker<'program, 'state>),
    {
        for idx in 0..checker_count {
            let mut slot_cb = |mut active: ActiveChecker<'program, '_, '_>| {
                cb(idx, active.checker());
            };
            self.with_slot_exclusive(program, idx, &mut slot_cb);
        }
    }

    pub(crate) fn get_global_diagnostics(&self, program: &Program) -> Vec<ast::Diagnostic> {
        let global_diagnostics = (0..self.checker_count())
            .map(|_| Mutex::new(Vec::new()))
            .collect::<Vec<_>>();
        self.for_each_checker_parallel(program, |idx, checker| {
            *global_diagnostics[idx]
                .lock()
                .unwrap_or_else(|err| err.into_inner()) = checker.get_global_diagnostics();
        });
        sort_and_deduplicate_diagnostics(
            global_diagnostics
                .into_iter()
                .flat_map(|diagnostics| {
                    diagnostics
                        .into_inner()
                        .unwrap_or_else(|err| err.into_inner())
                })
                .collect(),
        )
    }

    pub(crate) fn for_each_checker_group_do<'program, F>(
        &'program self,
        program: &'program Program,
        ctx: &core::Context,
        files: &[&'program ast::SourceFile],
        single_threaded: bool,
        cb: F,
    ) where
        F: for<'checker, 'state> Fn(
                &'checker mut checker::Checker<'program, 'state>,
                usize,
                &'program ast::SourceFile,
            ) + Sync,
    {
        let _ = ctx;
        let checker_count = self.checker_count();
        let file_groups =
            group_file_indexes_by_checker(
                checker_count,
                files.iter().copied().enumerate().map(|(file_index, file)| {
                    (file_index, self.checker_index_for_file(program, file))
                }),
            );

        if single_threaded || checker_count == 1 {
            self.for_each_checker_group_do_serial(program, files, file_groups, &cb);
            return;
        }

        // forEachCheckerGroupDo runs one task per checker in parallel. Each task
        // iterates the provided files, processing only those assigned to its checker.
        // Within each checker's set, files are visited in their original order.
        std::thread::scope(|scope| {
            for (checker_idx, file_group) in file_groups.into_iter().enumerate() {
                let cb = &cb;
                scope.spawn(move || {
                    let mut slot_cb = |mut active: ActiveChecker<'program, '_, '_>| {
                        let checker = active.checker();
                        for file_index in file_group.iter().copied() {
                            cb(checker, file_index, files[file_index]);
                        }
                    };
                    self.with_slot_exclusive(program, checker_idx, &mut slot_cb);
                });
            }
        });
    }

    fn for_each_checker_group_do_serial<'program, F>(
        &'program self,
        program: &'program Program,
        files: &[&'program ast::SourceFile],
        file_groups: Vec<Vec<usize>>,
        cb: &F,
    ) where
        F: for<'checker, 'state> Fn(
            &'checker mut checker::Checker<'program, 'state>,
            usize,
            &'program ast::SourceFile,
        ),
    {
        for (checker_idx, file_group) in file_groups.into_iter().enumerate() {
            let mut slot_cb = |mut active: ActiveChecker<'program, '_, '_>| {
                let checker = active.checker();
                for file_index in file_group.iter().copied() {
                    cb(checker, file_index, files[file_index]);
                }
            };
            self.with_slot_exclusive(program, checker_idx, &mut slot_cb);
        }
    }
}

fn group_file_indexes_by_checker(
    checker_count: usize,
    file_checker_indexes: impl IntoIterator<Item = (usize, usize)>,
) -> Vec<Vec<usize>> {
    let mut groups = vec![Vec::new(); checker_count];
    for (file_index, checker_index) in file_checker_indexes {
        groups
            .get_mut(checker_index)
            .expect("checker index must be in bounds")
            .push(file_index);
    }
    groups
}

fn with_shared_state<'program>(
    program: &'program Program,
    pool_id: usize,
    slot_index: usize,
    semantic_state: &SharedCheckerState,
    replace_if_canceled: bool,
    cb: &mut CheckerCallback<'program, '_>,
) {
    assert_not_reentrant(pool_id, slot_index);
    let mut semantic_state = semantic_state.lock().unwrap_or_else(|err| err.into_inner());
    let replacement_state = {
        let _active_slot = ActivePoolSlotGuard::new(pool_id, slot_index);
        let mut checker =
            checker::Checker::new_checker_with_state(program, None, &mut semantic_state);
        cb(ActiveChecker::new(program, &mut checker));
        if replace_if_canceled && checker.was_canceled() {
            Some(next_generation_state(&checker))
        } else {
            None
        }
    };
    if let Some(replacement_state) = replacement_state {
        *semantic_state = replacement_state;
    }
}

fn next_generation_state(checker: &checker::Checker<'_, '_>) -> checker::CheckerState {
    checker.next_generation_state()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send<T: Send>() {}

    fn assert_sync<T: Sync>() {}

    #[test]
    fn checker_parallelism_primitives_are_thread_safe() {
        // TypeScript-Go runs one worker per checker slot. These assertions keep
        // the Rust pool honest as we make that execution model real: per-slot
        // checker state may move behind a mutex, while the program/source graph
        // must be safe to share read-only between workers.
        assert_send::<checker::CheckerState>();
        assert_send::<SharedCheckerState>();
        assert_sync::<SharedCheckerState>();
        assert_send::<CheckerSlot>();
        assert_sync::<CheckerSlot>();
        assert_send::<Box<dyn CheckerPool>>();
        assert_sync::<Box<dyn CheckerPool>>();
        assert_send::<CheckerPoolImpl>();
        assert_sync::<CheckerPoolImpl>();
        assert_send::<crate::CreateCheckerPool>();
        assert_sync::<crate::CreateCheckerPool>();
        assert_send::<ts_module::ResolutionHostBox>();
        assert_sync::<ts_module::ResolutionHostBox>();
        assert_send::<ts_module::Resolver>();
        assert_sync::<ts_module::Resolver>();
        assert_send::<crate::ProcessedFiles>();
        assert_sync::<crate::ProcessedFiles>();
        assert_send::<crate::ProgramOptions>();
        assert_sync::<crate::ProgramOptions>();
        assert_sync::<Program>();
        assert_sync::<ast::SourceFile>();
    }

    #[test]
    fn active_pool_slot_guard_is_scoped() {
        let pool = 1;
        let slot = 2;
        {
            let _guard = ActivePoolSlotGuard::new(pool, slot);
            assert!(active_slot_for_current_thread(pool, slot));
        }
        assert!(!active_slot_for_current_thread(pool, slot));
    }

    #[test]
    #[should_panic(
        expected = "nested checker acquisition for an active checker slot requires CheckerAccess::Active"
    )]
    fn active_pool_slot_guard_rejects_reentrant_slot() {
        let pool = 3;
        let slot = 4;
        let _guard = ActivePoolSlotGuard::new(pool, slot);
        assert_not_reentrant(pool, slot);
    }

    #[test]
    fn checker_state_identity_slot_one_maps_to_pool_index_zero() {
        let state = checker::CheckerState::new_for_slot_index(0);
        assert_eq!(state.identity().slot().get(), 1);
        assert_eq!(checker_slot_index_from_state_identity(state.identity()), 0);
    }

    #[test]
    #[should_panic(expected = "checker state identity slot id must be one-based")]
    fn checker_state_identity_slot_zero_is_rejected() {
        let identity = checker::CheckerStateIdentity::new(
            checker::CheckerSlotId::new(0),
            checker::CheckerGeneration::initial(),
        );
        let _ = checker_slot_index_from_state_identity(identity);
    }

    #[test]
    fn group_file_indexes_by_checker_preserves_file_order_within_checker() {
        let groups =
            group_file_indexes_by_checker(3, [(0, 1), (1, 2), (2, 1), (3, 0), (4, 2), (5, 1)]);

        assert_eq!(groups, vec![vec![3], vec![0, 2, 5], vec![1, 4]]);
    }

    #[test]
    fn group_file_indexes_by_checker_keeps_empty_checker_groups() {
        let groups = group_file_indexes_by_checker(4, [(0, 2), (1, 2)]);

        assert_eq!(groups, vec![vec![], vec![], vec![0, 1], vec![]]);
    }

    #[test]
    fn for_each_checker_parallel_runs_checkers_in_parallel() {
        let program = test_program_with_checker_count(2);
        let pool = program
            .compiler_checker_pool
            .as_ref()
            .expect("built-in checker pool");
        let visits = std::sync::Mutex::new(Vec::new());

        pool.for_each_checker_parallel(&program, |checker_index, checker| {
            visits.lock().unwrap_or_else(|err| err.into_inner()).push((
                std::thread::current().id(),
                checker_index,
                checker_slot_index_from_state_identity(checker.state_identity()),
            ));
        });

        let visits = visits.into_inner().unwrap_or_else(|err| err.into_inner());
        let visited_threads = visits
            .iter()
            .map(|(thread_id, _, _)| *thread_id)
            .collect::<std::collections::HashSet<_>>();
        let mut checker_indexes = visits
            .into_iter()
            .map(|(_, checker_index, state_index)| {
                assert_eq!(checker_index, state_index);
                checker_index
            })
            .collect::<Vec<_>>();
        checker_indexes.sort_unstable();

        assert_eq!(visited_threads.len(), 2);
        assert_eq!(checker_indexes, vec![0, 1]);
    }

    #[test]
    fn for_each_checker_parallel_respects_single_threaded_fallback() {
        let program = test_program_with_checker_count_and_single_threaded(2, true);
        let pool = program
            .compiler_checker_pool
            .as_ref()
            .expect("built-in checker pool");
        let caller_thread = std::thread::current().id();
        let visits = std::sync::Mutex::new(Vec::new());

        pool.for_each_checker_parallel(&program, |checker_index, checker| {
            visits.lock().unwrap_or_else(|err| err.into_inner()).push((
                std::thread::current().id(),
                checker_index,
                checker_slot_index_from_state_identity(checker.state_identity()),
            ));
        });

        let visits = visits.into_inner().unwrap_or_else(|err| err.into_inner());

        assert_eq!(pool.checker_count(), 1);
        assert_eq!(visits, vec![(caller_thread, 0, 0)]);
    }

    #[test]
    fn for_each_checker_group_do_runs_checker_groups_in_parallel() {
        let program = test_program_with_checker_count(2);
        let pool = program
            .compiler_checker_pool
            .as_ref()
            .expect("built-in checker pool");
        let files: Vec<_> = program
            .source_files
            .iter()
            .map(|file| file.as_source_file())
            .collect();
        let visits = std::sync::Mutex::new(Vec::new());

        pool.for_each_checker_group_do(
            &program,
            &core::Context::default(),
            &files,
            false,
            |checker, file_index, _file| {
                let checker_index =
                    checker_slot_index_from_state_identity(checker.state_identity());
                visits.lock().unwrap_or_else(|err| err.into_inner()).push((
                    std::thread::current().id(),
                    checker_index,
                    file_index,
                ));
            },
        );

        let visits = visits.into_inner().unwrap_or_else(|err| err.into_inner());
        let visited_threads: std::collections::HashSet<_> =
            visits.iter().map(|(thread_id, _, _)| *thread_id).collect();
        let mut files_by_checker = std::collections::HashMap::<usize, Vec<usize>>::new();
        for (_, checker_index, file_index) in visits {
            files_by_checker
                .entry(checker_index)
                .or_default()
                .push(file_index);
        }

        assert_eq!(visited_threads.len(), 2);
        assert_eq!(files_by_checker.get(&0), Some(&vec![0, 2]));
        assert_eq!(files_by_checker.get(&1), Some(&vec![1, 3]));
    }

    #[test]
    fn for_each_checker_group_do_respects_single_threaded_fallback() {
        let program = test_program_with_checker_count(2);
        let pool = program
            .compiler_checker_pool
            .as_ref()
            .expect("built-in checker pool");
        let files: Vec<_> = program
            .source_files
            .iter()
            .map(|file| file.as_source_file())
            .collect();
        let caller_thread = std::thread::current().id();
        let visits = std::sync::Mutex::new(Vec::new());

        pool.for_each_checker_group_do(
            &program,
            &core::Context::default(),
            &files,
            true,
            |checker, file_index, _file| {
                let checker_index =
                    checker_slot_index_from_state_identity(checker.state_identity());
                visits.lock().unwrap_or_else(|err| err.into_inner()).push((
                    std::thread::current().id(),
                    checker_index,
                    file_index,
                ));
            },
        );

        let visits = visits.into_inner().unwrap_or_else(|err| err.into_inner());
        let mut files_by_checker = std::collections::HashMap::<usize, Vec<usize>>::new();
        for (thread_id, checker_index, file_index) in visits {
            assert_eq!(thread_id, caller_thread);
            files_by_checker
                .entry(checker_index)
                .or_default()
                .push(file_index);
        }

        assert_eq!(files_by_checker.get(&0), Some(&vec![0, 2]));
        assert_eq!(files_by_checker.get(&1), Some(&vec![1, 3]));
    }

    fn test_program_with_checker_count(checker_count: usize) -> Program {
        test_program_with_checker_count_and_single_threaded(checker_count, false)
    }

    fn test_program_with_checker_count_and_single_threaded(
        checker_count: usize,
        single_threaded: bool,
    ) -> Program {
        let file_names = ["a.ts", "b.ts", "c.ts", "d.ts"]
            .into_iter()
            .map(|name| format!("c:/src/{name}"))
            .collect::<Vec<_>>();
        let fs = ts_vfs::vfstest::from_map(
            file_names
                .iter()
                .map(|file_name| (file_name.as_str(), "export const value = 1;")),
            false,
        );
        let host: std::sync::Arc<dyn crate::CompilerHost> = crate::new_compiler_host(
            "c:/src".to_string(),
            Box::new(fs),
            "c:/lib".to_string(),
            None,
            None,
        )
        .into();
        let mut config = ts_tsoptions::ParsedCommandLine {
            file_names,
            ..Default::default()
        };
        config.set_compiler_options(core::CompilerOptions {
            no_lib: core::TS_TRUE,
            checkers: Some(checker_count),
            ..Default::default()
        });

        crate::new_program(crate::ProgramOptions {
            host,
            config: Box::new(config),
            use_source_of_project_reference: false,
            single_threaded: if single_threaded {
                core::Tristate::True
            } else {
                core::Tristate::default()
            },
            create_checker_pool: None,
            typings_location: String::new(),
            project_name: String::new(),
            type_script_version: String::new(),
            tracing: None,
        })
    }
}
