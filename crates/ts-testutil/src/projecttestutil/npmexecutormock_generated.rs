pub type NpmInstallFunc = Box<dyn Fn(&str, &[String]) -> Result<Vec<u8>, String> + Send + Sync>;

#[derive(Default)]
pub struct NpmExecutorMock {
    pub npm_install_func: Option<NpmInstallFunc>,
    npm_install_calls: Vec<NpmInstallCall>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NpmInstallCall {
    pub cwd: String,
    pub args: Vec<String>,
}

impl NpmExecutorMock {
    pub fn npm_install(&mut self, cwd: &str, args: &[String]) -> Result<Vec<u8>, String> {
        self.npm_install_calls.push(NpmInstallCall {
            cwd: cwd.to_string(),
            args: args.to_vec(),
        });
        if let Some(f) = &self.npm_install_func {
            return f(cwd, args);
        }
        Ok(Vec::new())
    }

    pub fn npm_install_calls(&self) -> Vec<NpmInstallCall> {
        self.npm_install_calls.clone()
    }
}

// --------------------------------------------------------------------------
// PORT STATUS
//   source:     internal/testutil/projecttestutil/npmexecutormock_generated.go (86 lines)
//   confidence: medium
//   todos:      bind to final ata::NpmExecutor trait
//   notes:      preserves generated callback, default empty output, call
//               recording, and calls accessor
// --------------------------------------------------------------------------
