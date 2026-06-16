use std::{collections::HashMap, sync::Mutex};

use ts_compiler as compiler;

pub type Program = compiler::Program;

pub struct ProgramCounter {
    refs: Mutex<HashMap<usize, i32>>,
}

impl Default for ProgramCounter {
    fn default() -> Self {
        Self {
            refs: Mutex::new(HashMap::new()),
        }
    }
}

impl ProgramCounter {
    // Ref increments the reference count for a program. If the program is not
    // yet tracked, it is added with a reference count of 1.
    pub fn r#ref(&self, program: *const Program) {
        let mut refs = self.refs.lock().unwrap_or_else(|err| err.into_inner());
        *refs.entry(program as usize).or_insert(0) += 1;
    }

    pub fn deref(&self, program: *const Program) -> bool {
        let mut refs = self.refs.lock().unwrap_or_else(|err| err.into_inner());
        let key = program as usize;
        let Some(count) = refs.get_mut(&key) else {
            return false;
        };
        *count -= 1;
        if *count < 0 {
            panic!("program reference count went below zero");
        }
        if *count == 0 {
            refs.remove(&key);
            return true;
        }
        false
    }

    pub fn len(&self) -> usize {
        self.refs
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .len()
    }
}
