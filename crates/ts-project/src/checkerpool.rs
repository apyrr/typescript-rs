use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};

use ts_ast as ast;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;

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
                panic!("active checker stack corrupted by out-of-order checker release");
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

fn active_pool_for_current_thread(pool: usize) -> bool {
    ACTIVE_POOL_SLOTS.with_borrow(|slots| slots.iter().any(|active| active.pool == pool))
}

fn assert_not_reentrant(pool: usize, slot: usize) {
    if active_slot_for_current_thread(pool, slot) {
        panic!(
            "nested checker acquisition for an active checker slot requires CheckerAccess::Active"
        );
    }
}

struct PoolState {
    active_slots: Vec<bool>,
    file_associations: HashMap<checker::SourceFileIdentity, usize>,
    request_associations: HashMap<String, usize>,
    global_diag_accumulated: Vec<ast::Diagnostic>,
    global_diag_changed: bool,
    global_diag_checker_count: Vec<usize>,
}

impl PoolState {
    fn new(max_checkers: usize) -> Self {
        Self {
            active_slots: vec![false; max_checkers],
            file_associations: HashMap::new(),
            request_associations: HashMap::new(),
            global_diag_accumulated: Vec::new(),
            global_diag_changed: false,
            global_diag_checker_count: vec![0; max_checkers],
        }
    }
}

struct SharedPool {
    state: Mutex<PoolState>,
    cond: Condvar,
}

impl SharedPool {
    fn new(max_checkers: usize) -> Self {
        Self {
            state: Mutex::new(PoolState::new(max_checkers)),
            cond: Condvar::new(),
        }
    }
}

struct CheckerSlot {
    semantic_state: Arc<Mutex<checker::CheckerState>>,
}

impl CheckerSlot {
    fn new(slot_index: usize) -> Self {
        Self {
            semantic_state: Arc::new(Mutex::new(checker::CheckerState::new_for_slot_index(
                slot_index,
            ))),
        }
    }
}

pub struct CheckerPool {
    shared: Arc<SharedPool>,
    slots: Vec<CheckerSlot>,
    log: Arc<dyn Fn(String) + Send + Sync>,
}

pub fn new_checker_pool(
    max_checkers: i32,
    log: Option<impl Fn(String) + Send + Sync + 'static>,
) -> CheckerPool {
    let max_checkers = usize::try_from(max_checkers)
        .expect("negative maxCheckers")
        .max(1);
    let slots = (0..max_checkers).map(CheckerSlot::new).collect();
    CheckerPool {
        shared: Arc::new(SharedPool::new(max_checkers)),
        slots,
        log: log
            .map(|log| Arc::new(log) as Arc<dyn Fn(String) + Send + Sync>)
            .unwrap_or_else(|| Arc::new(|_msg| {})),
    }
}

impl compiler::CheckerPool for CheckerPool {
    fn with_checker<'program>(
        &'program self,
        program: &'program compiler::Program,
        ctx: &core::Context,
        file: Option<&'program ast::SourceFile>,
        cb: &mut compiler::CheckerCallback<'program, '_>,
    ) {
        let request_id = core::get_request_id(ctx);
        let slot_index = self.acquire_checker_slot(file, &request_id);
        let semantic_state = self.shared_state_for_slot(slot_index);
        let mut release = ActiveProjectChecker::new(
            self.shared.clone(),
            self.log.clone(),
            request_id,
            slot_index,
        );
        let mut semantic_state = semantic_state.lock().unwrap_or_else(|err| err.into_inner());
        let replacement_state = {
            let _active_slot = ActivePoolSlotGuard::new(self.pool_id(), slot_index);
            let mut checker =
                checker::Checker::new_checker_with_state(program, None, &mut semantic_state);
            cb(compiler::ActiveChecker::new(program, &mut checker));
            release.release_with_checker(&mut checker)
        };
        if let Some(replacement_state) = replacement_state {
            *semantic_state = replacement_state;
        }
    }

    fn with_checker_for_state_identity<'program>(
        &'program self,
        program: &'program compiler::Program,
        ctx: &core::Context,
        identity: checker::CheckerStateIdentity,
        cb: &mut compiler::CheckerCallback<'program, '_>,
    ) {
        let request_id = core::get_request_id(ctx);
        let slot_index = self.acquire_checker_slot_by_index(
            compiler::checker_slot_index_from_state_identity(identity),
            &request_id,
        );
        let semantic_state = self.shared_state_for_slot(slot_index);
        let mut release = ActiveProjectChecker::new(
            self.shared.clone(),
            self.log.clone(),
            request_id,
            slot_index,
        );
        let mut semantic_state = semantic_state.lock().unwrap_or_else(|err| err.into_inner());
        let replacement_state = {
            let _active_slot = ActivePoolSlotGuard::new(self.pool_id(), slot_index);
            let mut checker =
                checker::Checker::new_checker_with_state(program, None, &mut semantic_state);
            cb(compiler::ActiveChecker::new(program, &mut checker));
            release.release_with_checker(&mut checker)
        };
        if let Some(replacement_state) = replacement_state {
            *semantic_state = replacement_state;
        }
    }
}

impl CheckerPool {
    fn pool_id(&self) -> usize {
        self as *const Self as usize
    }

    fn acquire_checker_slot(&self, file: Option<&ast::SourceFile>, request_id: &str) -> usize {
        let pool_id = self.pool_id();
        let file_key = file.map(source_file_key);
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        loop {
            if !request_id.is_empty() {
                if let Some(slot_index) = state.request_associations.get(request_id).copied() {
                    if !state.active_slots[slot_index] {
                        assert_not_reentrant(pool_id, slot_index);
                        state.active_slots[slot_index] = true;
                        return slot_index;
                    }
                    panic!("same-request checker reentry requires active checker threading");
                }
            }

            if let Some(file_key) = file_key {
                if let Some(slot_index) = state.file_associations.get(&file_key).copied() {
                    if !state.active_slots[slot_index] {
                        assert_not_reentrant(pool_id, slot_index);
                        state.active_slots[slot_index] = true;
                        if !request_id.is_empty() {
                            state
                                .request_associations
                                .insert(request_id.to_string(), slot_index);
                        }
                        return slot_index;
                    }
                }
            }

            if let Some(slot_index) = state.active_slots.iter().position(|active| !*active) {
                assert_not_reentrant(pool_id, slot_index);
                state.active_slots[slot_index] = true;
                if let Some(file_key) = file_key {
                    state.file_associations.insert(file_key, slot_index);
                }
                if !request_id.is_empty() {
                    state
                        .request_associations
                        .insert(request_id.to_string(), slot_index);
                }
                return slot_index;
            }

            if active_pool_for_current_thread(pool_id) {
                panic!(
                    "nested checker acquisition while all checker slots are active requires CheckerAccess::Active"
                );
            }
            (self.log)("checkerpool: Waiting for an available checker".to_string());
            state = self
                .shared
                .cond
                .wait(state)
                .unwrap_or_else(|err| err.into_inner());
        }
    }

    fn acquire_checker_slot_by_index(&self, slot_index: usize, request_id: &str) -> usize {
        let pool_id = self.pool_id();
        if slot_index >= self.slots.len() {
            panic!("checker slot index must be in bounds");
        }
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        loop {
            if !request_id.is_empty() {
                if let Some(active_slot) = state.request_associations.get(request_id).copied() {
                    if active_slot != slot_index {
                        panic!("request checker association does not match requested checker slot");
                    }
                    if !state.active_slots[slot_index] {
                        assert_not_reentrant(pool_id, slot_index);
                        state.active_slots[slot_index] = true;
                        return slot_index;
                    }
                    panic!("same-request checker reentry requires active checker threading");
                }
            }

            if !state.active_slots[slot_index] {
                assert_not_reentrant(pool_id, slot_index);
                state.active_slots[slot_index] = true;
                if !request_id.is_empty() {
                    state
                        .request_associations
                        .insert(request_id.to_string(), slot_index);
                }
                return slot_index;
            }

            if active_pool_for_current_thread(pool_id) {
                panic!(
                    "nested checker acquisition while the requested checker slot is active requires CheckerAccess::Active"
                );
            }
            (self.log)("checkerpool: Waiting for an available checker".to_string());
            state = self
                .shared
                .cond
                .wait(state)
                .unwrap_or_else(|err| err.into_inner());
        }
    }

    pub fn handle(&self) -> CheckerPoolHandle {
        CheckerPoolHandle {
            shared: self.shared.clone(),
        }
    }

    fn shared_state_for_slot(&self, slot_index: usize) -> Arc<Mutex<checker::CheckerState>> {
        self.slots
            .get(slot_index)
            .expect("checker slot index must be in bounds")
            .semantic_state
            .clone()
    }
}

struct ActiveProjectChecker {
    shared: Arc<SharedPool>,
    log: Arc<dyn Fn(String) + Send + Sync>,
    request_id: Option<String>,
    slot_index: usize,
    released: bool,
}

impl ActiveProjectChecker {
    fn new(
        shared: Arc<SharedPool>,
        log: Arc<dyn Fn(String) + Send + Sync>,
        request_id: String,
        slot_index: usize,
    ) -> Self {
        Self {
            shared,
            log,
            request_id: Some(request_id),
            slot_index,
            released: false,
        }
    }

    fn release_with_checker(
        &mut self,
        checker: &mut checker::Checker<'_, '_>,
    ) -> Option<checker::CheckerState> {
        let request_id = self.request_id.take().unwrap_or_default();
        self.released = true;
        release_checker(
            self.shared.clone(),
            self.log.clone(),
            request_id,
            self.slot_index,
            checker,
        )
    }
}

impl Drop for ActiveProjectChecker {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        let request_id = self.request_id.take().unwrap_or_default();
        release_checker_slot_without_diagnostics(self.shared.clone(), request_id, self.slot_index);
    }
}

fn release_checker(
    shared: Arc<SharedPool>,
    log: Arc<dyn Fn(String) + Send + Sync>,
    request_id: String,
    slot_index: usize,
    checker: &mut checker::Checker<'_, '_>,
) -> Option<checker::CheckerState> {
    let mut state = shared.state.lock().unwrap_or_else(|err| err.into_inner());
    if !request_id.is_empty() {
        state.request_associations.remove(&request_id);
    }
    let mut replacement_state = None;
    if checker.was_canceled() {
        log(format!(
            "checkerpool: Checker for request {} was canceled, disposing it",
            request_id
        ));
        state.global_diag_checker_count[slot_index] = 0;
        replacement_state = Some(next_generation_state(checker));
    } else {
        let globals = checker.get_global_diagnostics();
        if globals.len() == state.global_diag_checker_count[slot_index] {
            let active_slot = state
                .active_slots
                .get_mut(slot_index)
                .expect("released checker slot index must be in bounds");
            if !*active_slot {
                panic!("checker slot released without matching acquire");
            }
            *active_slot = false;
            shared.cond.notify_all();
            return replacement_state;
        }
        state.global_diag_checker_count[slot_index] = globals.len();
        let before = state.global_diag_accumulated.len();
        state.global_diag_accumulated = compiler::sort_and_deduplicate_diagnostics(
            state
                .global_diag_accumulated
                .iter()
                .cloned()
                .chain(globals)
                .collect(),
        );
        if state.global_diag_accumulated.len() != before {
            state.global_diag_changed = true;
        }
    }
    let active_slot = state
        .active_slots
        .get_mut(slot_index)
        .expect("released checker slot index must be in bounds");
    if !*active_slot {
        panic!("checker slot released without matching acquire");
    }
    *active_slot = false;
    shared.cond.notify_all();
    replacement_state
}

fn release_checker_slot_without_diagnostics(
    shared: Arc<SharedPool>,
    request_id: String,
    slot_index: usize,
) {
    let mut state = shared.state.lock().unwrap_or_else(|err| err.into_inner());
    if !request_id.is_empty() {
        state.request_associations.remove(&request_id);
    }
    if let Some(active_slot) = state.active_slots.get_mut(slot_index) {
        *active_slot = false;
    }
    shared.cond.notify_all();
}

#[derive(Clone)]
pub struct CheckerPoolHandle {
    shared: Arc<SharedPool>,
}

impl CheckerPoolHandle {
    pub fn get_global_diagnostics(&self) -> Vec<ast::Diagnostic> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .global_diag_accumulated
            .clone()
    }

    pub fn take_new_global_diagnostics(&self) -> bool {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let changed = state.global_diag_changed;
        state.global_diag_changed = false;
        changed
    }
}

fn next_generation_state(checker: &checker::Checker<'_, '_>) -> checker::CheckerState {
    checker.next_generation_state()
}

fn source_file_key(file: &ast::SourceFile) -> checker::SourceFileIdentity {
    checker::SourceFileIdentity::from_source_file(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_checker_slot_reuses_inactive_same_request_checker() {
        let pool = new_checker_pool(1, Option::<fn(String)>::None);
        {
            let mut state = pool.shared.state.lock().unwrap();
            state.request_associations.insert("request".to_string(), 0);
        }

        assert_eq!(pool.acquire_checker_slot(None, "request"), 0);
    }

    #[test]
    #[should_panic(expected = "same-request checker reentry requires active checker threading")]
    fn acquire_checker_slot_rejects_active_same_request_reentry() {
        let pool = new_checker_pool(1, Option::<fn(String)>::None);
        assert_eq!(pool.acquire_checker_slot(None, "request"), 0);
        let _ = pool.acquire_checker_slot(None, "request");
    }

    #[test]
    #[should_panic(
        expected = "nested checker acquisition while all checker slots are active requires CheckerAccess::Active"
    )]
    fn acquire_checker_slot_rejects_same_thread_wait_deadlock() {
        let pool = new_checker_pool(1, Option::<fn(String)>::None);
        let _guard = ActivePoolSlotGuard::new(pool.pool_id(), 0);
        {
            let mut state = pool.shared.state.lock().unwrap();
            state.active_slots[0] = true;
        }

        let _ = pool.acquire_checker_slot(None, "");
    }

    #[test]
    fn active_pool_slot_guard_releases_current_thread_slot() {
        let pool = new_checker_pool(1, Option::<fn(String)>::None);
        {
            let _guard = ActivePoolSlotGuard::new(pool.pool_id(), 0);
            assert!(active_slot_for_current_thread(pool.pool_id(), 0));
        }

        assert!(!active_slot_for_current_thread(pool.pool_id(), 0));
    }

    #[test]
    fn checker_state_identity_slot_one_maps_to_pool_index_zero() {
        let pool = new_checker_pool(1, Option::<fn(String)>::None);
        let state = checker::CheckerState::new_for_slot_index(0);
        let slot_index = compiler::checker_slot_index_from_state_identity(state.identity());

        assert_eq!(state.identity().slot().get(), 1);
        assert_eq!(slot_index, 0);
        assert_eq!(pool.acquire_checker_slot_by_index(slot_index, ""), 0);
    }

    #[test]
    fn drop_release_clears_active_slot_and_request_association() {
        let pool = new_checker_pool(1, Option::<fn(String)>::None);
        {
            let mut state = pool.shared.state.lock().unwrap();
            state.active_slots[0] = true;
            state.request_associations.insert("request".to_string(), 0);
        }

        let release = ActiveProjectChecker::new(
            pool.shared.clone(),
            pool.log.clone(),
            "request".to_string(),
            0,
        );
        drop(release);

        let state = pool.shared.state.lock().unwrap();
        assert!(!state.active_slots[0]);
        assert!(!state.request_associations.contains_key("request"));
    }
}
