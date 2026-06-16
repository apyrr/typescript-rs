use std::{
    collections::HashMap,
    io::{Read, Write},
    panic::{self, AssertUnwindSafe},
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
        mpsc,
    },
};

use ts_core as context;
use ts_json as json;
use ts_jsonrpc as jsonrpc;

use crate::{CallbackClient, Conn, Error, Handler, Message, Protocol, ReadWriteClose};

// AsyncConn manages bidirectional JSON-RPC communication.
pub struct AsyncConn {
    rwc: Box<dyn ReadWriteClose>,
    protocol: Arc<dyn Protocol + Send + Sync>,
    handler: Arc<dyn Handler>,

    // For server->client requests
    seq: Arc<AtomicI64>,
    pending: Arc<Mutex<HashMap<jsonrpc::Id, mpsc::SyncSender<Message>>>>,
    write_mu: Arc<Mutex<()>>,
}

// NewAsyncConn creates a new async connection with the given transport and handler.
// It uses JSONRPCProtocol (LSP-style Content-Length framing) by default.
pub fn new_async_conn(rwc: Box<dyn ReadWriteClose>, handler: Arc<dyn Handler>) -> AsyncConn {
    let protocol: Arc<dyn Protocol + Send + Sync> = Arc::new(AsyncJsonRpcProtocol::new(
        CloneReadWriteClose::new(rwc.clone_reader_writer()),
    ));
    new_async_conn_with_protocol(rwc, protocol, handler)
}

struct AsyncJsonRpcProtocol<RW: Read + Write>(
    crate::JSONRPCProtocol<crate::protocol_jsonrpc::SharedReadWriter<RW>>,
);

impl<RW> AsyncJsonRpcProtocol<RW>
where
    RW: Read + Write,
{
    fn new(rw: RW) -> Self {
        Self(crate::new_jsonrpc_protocol(rw))
    }
}

impl<RW> Protocol for AsyncJsonRpcProtocol<RW>
where
    RW: Read + Write + Send,
{
    fn read_message(&self) -> Result<Message, std::io::Error> {
        self.0.read_message()
    }

    fn write_request(
        &self,
        id: Option<&jsonrpc::Id>,
        method: &str,
        params: json::Value,
    ) -> Result<(), std::io::Error> {
        self.0.write_request(id, method, params)
    }

    fn write_notification(&self, method: &str, params: json::Value) -> Result<(), std::io::Error> {
        self.0.write_notification(method, params)
    }

    fn write_response(
        &self,
        id: Option<&jsonrpc::Id>,
        result: json::Value,
    ) -> Result<(), std::io::Error> {
        self.0.write_response(id, result)
    }

    fn write_error(
        &self,
        id: Option<&jsonrpc::Id>,
        err: &jsonrpc::ResponseError,
    ) -> Result<(), std::io::Error> {
        self.0.write_error(id, err)
    }
}

struct CloneReadWriteClose {
    inner: Box<dyn ReadWriteClose>,
}

impl CloneReadWriteClose {
    fn new(inner: Box<dyn ReadWriteClose>) -> Self {
        Self { inner }
    }
}

impl Clone for CloneReadWriteClose {
    fn clone(&self) -> Self {
        Self::new(self.inner.clone_reader_writer())
    }
}

impl Read for CloneReadWriteClose {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for CloneReadWriteClose {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

// NewAsyncConnWithProtocol creates a new async connection with a custom protocol.
pub fn new_async_conn_with_protocol(
    rwc: Box<dyn ReadWriteClose>,
    protocol: Arc<dyn Protocol + Send + Sync>,
    handler: Arc<dyn Handler>,
) -> AsyncConn {
    AsyncConn {
        rwc,
        protocol,
        handler,
        seq: Arc::new(AtomicI64::new(0)),
        pending: Arc::new(Mutex::new(HashMap::new())),
        write_mu: Arc::new(Mutex::new(())),
    }
}

impl AsyncConn {
    // Run starts processing messages on the connection.
    // It blocks until the context is cancelled or an error occurs.
    pub fn run(&self, ctx: context::Context) -> Result<(), Error> {
        loop {
            if let Some(err) = ctx.err() {
                return Err(Error::new(err.to_string()));
            }

            let msg = match self.protocol.read_message() {
                Ok(msg) => msg,
                Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(err) => return Err(Error::new(err.to_string())),
            };

            if msg.is_response() {
                self.handle_response(msg);
            } else if msg.is_request() {
                self.handle_request(ctx.clone(), msg);
            } else if msg.is_notification() {
                self.handle_notification(ctx.clone(), msg);
            }
        }
    }

    // handleResponse matches a response to a pending request.
    fn handle_response(&self, msg: Message) {
        let ch = {
            let mut pending = self.pending.lock().unwrap_or_else(|err| err.into_inner());
            let Some(id) = msg.id.as_ref() else {
                return;
            };
            pending.remove(id)
        };

        if let Some(ch) = ch {
            let _ = ch.send(msg);
        }
    }

    // handleRequest processes an incoming request.
    fn handle_request(&self, ctx: context::Context, msg: Message) {
        handle_request_with(
            self.protocol.as_ref(),
            self.handler.as_ref(),
            &self.write_mu,
            ctx,
            msg,
        );
    }

    // handleNotification processes an incoming notification.
    fn handle_notification(&self, ctx: context::Context, msg: Message) {
        let _ = self
            .handler
            .handle_notification(&ctx, &msg.method, msg.params);
    }

    // Call sends a request to the client and waits for a response.
    pub fn call(
        &self,
        ctx: context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        call_with(
            self.protocol.as_ref(),
            &self.seq,
            &self.pending,
            &self.write_mu,
            ctx,
            method,
            params,
        )
    }

    // Notify sends a notification to the client (no response expected).
    pub fn notify(
        &self,
        _ctx: context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<(), Error> {
        let _write_guard = self.write_mu.lock().unwrap_or_else(|err| err.into_inner());
        self.protocol
            .write_notification(method, params)
            .map_err(|err| Error::new(err.to_string()))
    }

    pub fn callback_client(&self) -> AsyncCallbackClient {
        AsyncCallbackClient {
            protocol: Arc::clone(&self.protocol),
            seq: Arc::clone(&self.seq),
            pending: Arc::clone(&self.pending),
            write_mu: Arc::clone(&self.write_mu),
        }
    }
}

impl Conn for AsyncConn {
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

fn handle_request_with(
    protocol: &(dyn Protocol + Send + Sync),
    handler: &dyn Handler,
    write_mu: &Mutex<()>,
    ctx: context::Context,
    msg: Message,
) {
    let request_result = panic::catch_unwind(AssertUnwindSafe(|| {
        let write_result = handler.handle_request(&ctx, &msg.method, msg.params.clone());

        let _write_guard = write_mu.lock().unwrap_or_else(|err| err.into_inner());

        let write_err = match write_result {
            Err(err) => protocol.write_error(
                msg.id.as_ref(),
                &jsonrpc::ResponseError {
                    code: jsonrpc::CODE_INTERNAL_ERROR,
                    message: err.to_string(),
                    data: None,
                },
            ),
            Ok(result) => protocol.write_response(msg.id.as_ref(), result),
        };

        if let Err(write_err) = write_err {
            panic!("api: failed to write response: {write_err}");
        }
    }));

    // Recover from panics and convert to error response with stack trace.
    if let Err(payload) = request_result {
        let recovered = panic_message(payload);
        let err = format!(
            "panic: {recovered}\n{}",
            std::backtrace::Backtrace::force_capture()
        );
        let _write_guard = write_mu.lock().unwrap_or_else(|err| err.into_inner());
        let write_err = protocol.write_error(
            msg.id.as_ref(),
            &jsonrpc::ResponseError {
                code: jsonrpc::CODE_INTERNAL_ERROR,
                message: err,
                data: None,
            },
        );
        if let Err(write_err) = write_err {
            panic!(
                "api: failed to write panic error response: {write_err} (original panic: {recovered})"
            );
        }
    }
}

struct PendingCleanup {
    pending: Arc<Mutex<HashMap<jsonrpc::Id, mpsc::SyncSender<Message>>>>,
    id: jsonrpc::Id,
}

pub struct AsyncCallbackClient {
    protocol: Arc<dyn Protocol + Send + Sync>,
    seq: Arc<AtomicI64>,
    pending: Arc<Mutex<HashMap<jsonrpc::Id, mpsc::SyncSender<Message>>>>,
    write_mu: Arc<Mutex<()>>,
}

impl CallbackClient for AsyncCallbackClient {
    fn call(
        &self,
        ctx: &context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<json::Value, Error> {
        call_with(
            self.protocol.as_ref(),
            &self.seq,
            &self.pending,
            &self.write_mu,
            ctx.clone(),
            method,
            params,
        )
    }
}

fn call_with(
    protocol: &(dyn Protocol + Send + Sync),
    seq: &AtomicI64,
    pending: &Arc<Mutex<HashMap<jsonrpc::Id, mpsc::SyncSender<Message>>>>,
    write_mu: &Mutex<()>,
    ctx: context::Context,
    method: &str,
    params: json::Value,
) -> Result<json::Value, Error> {
    let id = jsonrpc::Id::new_string(format!("api{}", seq.fetch_add(1, Ordering::SeqCst) + 1));

    let (response_sender, response_receiver) = mpsc::sync_channel(1);
    {
        let mut pending = pending.lock().unwrap_or_else(|err| err.into_inner());
        pending.insert(id.clone(), response_sender);
    }

    let cleanup = PendingCleanup {
        pending: Arc::clone(pending),
        id: id.clone(),
    };

    let err = {
        let _write_guard = write_mu.lock().unwrap_or_else(|err| err.into_inner());
        protocol.write_request(Some(&id), method, params)
    };

    if let Err(err) = err {
        drop(cleanup);
        return Err(Error::new(err.to_string()));
    }

    loop {
        if let Some(err) = ctx.err() {
            drop(cleanup);
            return Err(Error::new(err.to_string()));
        }
        match response_receiver.recv_timeout(std::time::Duration::from_millis(10)) {
            Ok(resp) => {
                drop(cleanup);
                if let Some(resp_err) = resp.error {
                    return Err(Error::new(format!(
                        "api: remote error [{}]: {}",
                        resp_err.code, resp_err.message
                    )));
                }
                return Ok(resp.result);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                drop(cleanup);
                return Err(Error::ConnClosed);
            }
        }
    }
}

impl Drop for PendingCleanup {
    fn drop(&mut self) {
        let mut pending = self.pending.lock().unwrap_or_else(|err| err.into_inner());
        pending.remove(&self.id);
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
