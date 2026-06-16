use std::{
    panic::{self, AssertUnwindSafe},
    sync::{Arc, Mutex},
};

use ts_core as context;
use ts_json as json;
use ts_jsonrpc as jsonrpc;

use crate::{CallbackClient, Conn, Error, Handler, Message, Protocol, ReadWriteClose};

// SyncConn manages bidirectional communication with synchronous request handling.
// Requests are handled one at a time inline, and outgoing calls are serialized.
pub struct SyncConn {
    rwc: Mutex<Box<dyn ReadWriteClose>>,
    protocol: Arc<dyn Protocol + Send + Sync>,
    handler: Arc<dyn Handler>,

    // mu serializes all protocol operations (reads and writes).
    // This ensures that concurrent calls from handler goroutines (e.g., project code
    // spawning goroutines that invoke filesystem callbacks) don't corrupt the stream.
    mu: Arc<Mutex<()>>,
}

// NewSyncConn creates a new sync connection with the given transport and handler.
pub fn new_sync_conn(
    rwc: Box<dyn ReadWriteClose>,
    protocol: Arc<dyn Protocol + Send + Sync>,
    handler: Arc<dyn Handler>,
) -> SyncConn {
    SyncConn {
        rwc: Mutex::new(rwc),
        protocol,
        handler,
        mu: Arc::new(Mutex::new(())),
    }
}

impl SyncConn {
    // Run starts processing messages on the connection.
    // It blocks until the context is cancelled or an error occurs.
    pub fn run(&self, ctx: context::Context) -> Result<(), Error> {
        loop {
            if let Some(err) = ctx.err() {
                return Err(Error::new(err.to_string()));
            }

            let msg = {
                let _guard = self.mu.lock().unwrap_or_else(|err| err.into_inner());
                match self.protocol.read_message() {
                    Ok(msg) => msg,
                    Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
                    Err(err) => return Err(Error::new(err.to_string())),
                }
            };

            if msg.is_request() {
                self.handle_request(ctx.clone(), msg);
            } else if msg.is_notification() {
                self.handle_notification(ctx.clone(), msg);
            } else {
                // Responses are not expected in the main loop - they are read inline by Call().
                return Err(Error::new(
                    "api: unexpected response message in sync connection",
                ));
            }
        }
    }

    // handleRequest processes an incoming request.
    fn handle_request(&self, ctx: context::Context, msg: Message) {
        let request_result = panic::catch_unwind(AssertUnwindSafe(|| {
            let result = self
                .handler
                .handle_request(&ctx, &msg.method, msg.params.clone());

            let _guard = self.mu.lock().unwrap_or_else(|err| err.into_inner());

            let write_err = match result {
                Err(err) => self.protocol.write_error(
                    msg.id.as_ref(),
                    &jsonrpc::ResponseError {
                        code: jsonrpc::CODE_INTERNAL_ERROR,
                        message: err.to_string(),
                        data: None,
                    },
                ),
                Ok(result) => self.protocol.write_response(msg.id.as_ref(), result),
            };

            if let Err(write_err) = write_err {
                panic!("api: failed to write response: {write_err}");
            }
        }));

        if let Err(payload) = request_result {
            // Recover from panics and convert to error response with stack trace
            let panic_message = panic_message(payload);
            let err = format!(
                "panic: {panic_message}\n{}",
                std::backtrace::Backtrace::force_capture()
            );

            let _guard = self.mu.lock().unwrap_or_else(|err| err.into_inner());
            let write_err = self.protocol.write_error(
                msg.id.as_ref(),
                &jsonrpc::ResponseError {
                    code: jsonrpc::CODE_INTERNAL_ERROR,
                    message: err,
                    data: None,
                },
            );

            if let Err(write_err) = write_err {
                panic!(
                    "api: failed to write panic error response: {write_err} (original panic: {panic_message})"
                );
            }
        }
    }

    // handleNotification processes an incoming notification.
    fn handle_notification(&self, ctx: context::Context, msg: Message) {
        let _ = self
            .handler
            .handle_notification(&ctx, &msg.method, msg.params);
    }

    // Call sends a request to the client and waits for a response.
    // This method is safe to call from multiple goroutines - calls are serialized.
    pub fn call(
        &self,
        ctx: context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        call_with(self.protocol.as_ref(), &self.mu, ctx, method, params)
    }

    // Notify sends a notification to the client (no response expected).
    pub fn notify(
        &self,
        _ctx: context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<(), Error> {
        let _guard = self.mu.lock().unwrap_or_else(|err| err.into_inner());
        self.protocol
            .write_notification(method, params)
            .map_err(|err| Error::new(err.to_string()))
    }

    pub fn callback_client(&self) -> SyncCallbackClient {
        SyncCallbackClient {
            protocol: Arc::clone(&self.protocol),
            mu: Arc::clone(&self.mu),
        }
    }
}

impl Conn for SyncConn {
    fn run(&self, ctx: &context::Context) -> Result<(), Error> {
        self.run(ctx.clone())
    }

    fn call(
        &self,
        ctx: &context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        self.call(ctx.clone(), method, params)
    }

    fn notify(
        &self,
        ctx: &context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<(), Error> {
        self.notify(ctx.clone(), method, params)
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "recovered panic".to_string()
    }
}

pub struct SyncCallbackClient {
    protocol: Arc<dyn Protocol + Send + Sync>,
    mu: Arc<Mutex<()>>,
}

impl CallbackClient for SyncCallbackClient {
    fn call(
        &self,
        ctx: &context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        call_with(
            self.protocol.as_ref(),
            &self.mu,
            ctx.clone(),
            method,
            params,
        )
    }
}

fn call_with(
    protocol: &(dyn Protocol + Send + Sync),
    mu: &Mutex<()>,
    ctx: context::Context,
    method: &str,
    params: json::Value,
) -> Result<json::Value, Error> {
    let _guard = mu.lock().unwrap_or_else(|err| err.into_inner());
    let id = jsonrpc::Id::new_string(method.to_string());

    if let Err(err) = protocol.write_request(Some(&id), method, params) {
        return Err(Error::new(err.to_string()));
    }

    if let Some(err) = ctx.err() {
        return Err(Error::new(err.to_string()));
    }

    let msg = protocol
        .read_message()
        .map_err(|err| Error::new(err.to_string()))?;

    if msg.is_response()
        && msg.id.is_some()
        && msg.id.as_ref().map(|id| id.to_string()) == Some(method.to_string())
    {
        if let Some(resp_err) = msg.error {
            return Err(Error::new(format!(
                "api: remote error [{}]: {}",
                resp_err.code, resp_err.message
            )));
        }
        return Ok(msg.result);
    }

    Err(Error::new(format!(
        "api: unexpected message while waiting for {method:?} response"
    )))
}
