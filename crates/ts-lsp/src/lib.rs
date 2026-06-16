#![forbid(unsafe_code)]

pub use ts_lsproto as lsproto;

#[expect(
    dead_code,
    reason = "stderr logger constructor is kept for test/debug entry points"
)]
pub mod logger;
#[expect(
    dead_code,
    unused_assignments,
    reason = "ported progress scheduler state is ahead of current callers"
)]
pub mod progress;
pub mod server;
pub mod stack_sanitizer;

#[cfg(test)]
mod progress_test;
#[cfg(test)]
mod replay_test;
#[cfg(test)]
mod server_completion_test;
#[cfg(test)]
mod server_progress_test;
#[cfg(test)]
mod server_projectinfo_test;
#[cfg(test)]
mod server_projectreference_updates_test;
#[cfg(test)]
mod server_semantictokens_test;
#[cfg(test)]
mod server_shutdown_test;
#[cfg(test)]
mod stack_sanitizer_test;
#[cfg(test)]
mod testmain_test;

pub use logger::*;
pub use progress::*;
pub use server::*;
pub use stack_sanitizer::*;

#[cfg(any(test, feature = "test-support"))]
mod lsptestutil_bridge {
    use std::io;

    use ts_core::context;
    use ts_testutil::lsptestutil;

    use crate::{ServerOptions, new_server};

    impl crate::Reader for lsptestutil::LspReader {
        fn read(&self) -> Result<crate::lsproto::Message, io::Error> {
            self.read()
        }
    }

    impl crate::Writer for lsptestutil::LspWriter {
        fn write(&self, msg: &crate::lsproto::Message) -> Result<(), io::Error> {
            self.write(msg)
        }
    }

    impl lsptestutil::ServerOptionsExt for ServerOptions {
        fn into_test_server(
            mut self,
            input_reader: lsptestutil::LspReader,
            output_writer: lsptestutil::LspWriter,
        ) -> lsptestutil::TestServerParts {
            self.r#in = Some(Box::new(input_reader));
            self.out = Some(Box::new(output_writer));
            assert!(
                self.parse_cache.is_none(),
                "LSP test server cannot move a parse cache across threads"
            );
            let ServerOptions {
                r#in,
                out,
                err,
                cwd,
                fs,
                default_library_path,
                typings_location,
                parse_cache: _,
                compiler_options_for_inferred_projects,
                npm_install,
                progress_delay,
                set_parent_process_id,
            } = self;
            let init_complete = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let init_complete_for_thread = init_complete.clone();
            lsptestutil::TestServerParts {
                run_server: Box::new(move || {
                    let mut server = new_server(ServerOptions {
                        r#in,
                        out,
                        err,
                        cwd,
                        fs,
                        default_library_path,
                        typings_location,
                        parse_cache: None,
                        compiler_options_for_inferred_projects,
                        npm_install,
                        progress_delay,
                        set_parent_process_id,
                    });
                    server.init_complete_signal = init_complete_for_thread;
                    server.run(context::background())
                }),
                init_complete,
            }
        }
    }
}
