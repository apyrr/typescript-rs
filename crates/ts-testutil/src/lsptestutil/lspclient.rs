use std::borrow::Borrow;
use std::collections::HashMap;
use std::io;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;

use ts_jsonrpc as jsonrpc;
use ts_lsproto as lsproto;

pub type RequestMessage = lsproto::RequestMessage;
pub type ResponseMessage = lsproto::ResponseMessage;

pub struct TestServerParts {
    pub run_server: Box<dyn FnOnce() -> Result<(), io::Error> + Send>,
    pub init_complete: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct TestServerHandle {
    init_complete: Arc<AtomicBool>,
}

impl TestServerHandle {
    pub fn init_complete(&self) -> Receiver<()> {
        let (sender, receiver) = mpsc::sync_channel(1);
        if self.init_complete.load(Ordering::SeqCst) {
            let _ = sender.send(());
            return receiver;
        }
        let init_complete = self.init_complete.clone();
        thread::spawn(move || {
            while !init_complete.load(Ordering::SeqCst) {
                thread::sleep(std::time::Duration::from_millis(1));
            }
            let _ = sender.send(());
        });
        receiver
    }
}

pub trait ServerOptionsExt {
    fn into_test_server(self, input_reader: LspReader, output_writer: LspWriter)
    -> TestServerParts;
}

pub struct LspReader {
    receiver: Receiver<lsproto::Message>,
}

impl LspReader {
    pub fn read(&self) -> Result<lsproto::Message, io::Error> {
        self.receiver
            .recv()
            .map_err(|_| io::Error::new(io::ErrorKind::UnexpectedEof, "EOF"))
    }
}

#[derive(Clone)]
pub struct LspWriter {
    sender: Arc<Mutex<Option<SyncSender<lsproto::Message>>>>,
}

impl LspWriter {
    pub fn write(&self, msg: &lsproto::Message) -> Result<(), io::Error> {
        let sender = self
            .sender
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "EOF"))?;
        sender
            .send(msg.clone())
            .map_err(|err| io::Error::new(io::ErrorKind::BrokenPipe, err.to_string()))
    }

    pub fn close(&self) {
        *self.sender.lock().unwrap_or_else(|err| err.into_inner()) = None;
    }
}

pub fn new_lsp_pipe() -> (LspReader, LspWriter) {
    let (sender, receiver) = mpsc::sync_channel(100);
    (
        LspReader { receiver },
        LspWriter {
            sender: Arc::new(Mutex::new(Some(sender))),
        },
    )
}

pub type ServerRequestHandler = Box<dyn Fn(&RequestMessage) -> Option<ResponseMessage> + Send>;
pub type ServerNotificationHandler = Box<dyn Fn(&RequestMessage) + Send>;

type PendingRequests = Arc<Mutex<HashMap<jsonrpc::Id, SyncSender<ResponseMessage>>>>;
type SharedServerRequestHandler = Arc<Mutex<Option<ServerRequestHandler>>>;
type SharedServerNotificationHandler = Arc<Mutex<Option<ServerNotificationHandler>>>;

pub struct LSPClient {
    pub server: TestServerHandle,
    pub input_writer: LspWriter,
    pub id: i32,
    pub on_server_request: SharedServerRequestHandler,
    pub on_server_notification: SharedServerNotificationHandler,
    pub pending_requests: PendingRequests,
}

pub type LspClient = LSPClient;

pub trait RequestInfoArg<Params, Resp> {
    fn request_info(&self) -> &lsproto::RequestInfo<Params, Resp>;
}

impl<Params, Resp> RequestInfoArg<Params, Resp> for lsproto::RequestInfo<Params, Resp> {
    fn request_info(&self) -> &lsproto::RequestInfo<Params, Resp> {
        self
    }
}

impl<Params, Resp> RequestInfoArg<Params, Resp> for &lsproto::RequestInfo<Params, Resp> {
    fn request_info(&self) -> &lsproto::RequestInfo<Params, Resp> {
        self
    }
}

impl<Params, Resp> RequestInfoArg<Params, Resp> for LazyLock<lsproto::RequestInfo<Params, Resp>> {
    fn request_info(&self) -> &lsproto::RequestInfo<Params, Resp> {
        self.deref()
    }
}

pub trait NotificationInfoArg<Params> {
    fn notification_info(&self) -> &lsproto::NotificationInfo<Params>;
}

impl<Params> NotificationInfoArg<Params> for lsproto::NotificationInfo<Params> {
    fn notification_info(&self) -> &lsproto::NotificationInfo<Params> {
        self
    }
}

impl<Params> NotificationInfoArg<Params> for &lsproto::NotificationInfo<Params> {
    fn notification_info(&self) -> &lsproto::NotificationInfo<Params> {
        self
    }
}

impl<Params> NotificationInfoArg<Params> for LazyLock<lsproto::NotificationInfo<Params>> {
    fn notification_info(&self) -> &lsproto::NotificationInfo<Params> {
        self.deref()
    }
}

impl LSPClient {
    pub fn next_id(&mut self) -> i32 {
        let id = self.id;
        self.id += 1;
        id
    }

    pub fn set_on_server_notification(&mut self, handler: Option<ServerNotificationHandler>) {
        *self
            .on_server_notification
            .lock()
            .unwrap_or_else(|err| err.into_inner()) = handler;
    }

    pub fn message_router_once(
        output_reader: &LspReader,
        input_writer: &LspWriter,
        pending_requests: &PendingRequests,
        on_server_request: &SharedServerRequestHandler,
        on_server_notification: &SharedServerNotificationHandler,
    ) -> Result<bool, io::Error> {
        let msg = match output_reader.read() {
            Ok(msg) => msg,
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
            Err(err) => return Err(err),
        };
        match msg.kind {
            jsonrpc::MessageKind::Response => {
                Self::handle_response(pending_requests, msg.as_response().clone())
            }
            jsonrpc::MessageKind::Request => Self::handle_server_request(
                input_writer,
                on_server_request,
                msg.as_request().clone(),
            )?,
            jsonrpc::MessageKind::Notification => {
                if let Some(handler) = &*on_server_notification
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                {
                    handler(msg.as_request());
                }
            }
        }
        Ok(true)
    }

    pub fn handle_response(pending_requests: &PendingRequests, resp: ResponseMessage) {
        let Some(id) = resp.id.clone() else {
            return;
        };
        if let Some(sender) = pending_requests
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .remove(&id)
        {
            let _ = sender.send(resp);
        }
    }

    pub fn handle_server_request(
        input_writer: &LspWriter,
        on_server_request: &SharedServerRequestHandler,
        req: RequestMessage,
    ) -> Result<(), io::Error> {
        let response = on_server_request
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .as_ref()
            .and_then(|handler| handler(&req))
            .unwrap_or_else(|| ResponseMessage {
                id: req.id.clone(),
                jsonrpc: req.jsonrpc,
                result: serde_json::Value::Null,
                error: Some(jsonrpc::ResponseError {
                    code: lsproto::ErrorCodeMethodNotFound,
                    message: format!("Unknown method: {}", req.method),
                    data: None,
                }),
            });
        input_writer.write(&response.message())
    }

    pub fn write_msg(&self, msg: lsproto::Message) -> Result<(), io::Error> {
        self.input_writer.write(&msg)
    }

    pub fn send_request_json<Params, Resp>(
        &mut self,
        method: &str,
        params: Params,
    ) -> Result<Resp, String>
    where
        Params: serde::Serialize,
        Resp: serde::de::DeserializeOwned,
    {
        let id = self.next_id();
        let req_id = jsonrpc::Id::new_int(id);
        let req = RequestMessage {
            id: Some(req_id.clone()),
            jsonrpc: jsonrpc::JsonRpcVersion,
            method: method.to_string(),
            params: serde_json::to_value(params).map_err(|err| err.to_string())?,
        };
        let (_msg, resp, ok) = self.send_request_worker(&req, req_id);
        if !ok {
            return Err(format!("request {method} did not receive a response"));
        }
        if let Some(error) = resp.error {
            return Err(error.to_string());
        }
        serde_json::from_value(resp.result).map_err(|err| err.to_string())
    }

    pub fn send_request_worker(
        &mut self,
        req: &RequestMessage,
        req_id: jsonrpc::Id,
    ) -> (lsproto::Message, ResponseMessage, bool) {
        let receiver = self.start_request_worker(req, req_id);
        self.wait_for_response(receiver)
    }

    pub fn send_request_async(
        &mut self,
        req: &RequestMessage,
        req_id: jsonrpc::Id,
    ) -> Receiver<ResponseMessage> {
        self.start_request_worker(req, req_id)
    }

    pub fn wait_for_response(
        &mut self,
        receiver: Receiver<ResponseMessage>,
    ) -> (lsproto::Message, ResponseMessage, bool) {
        match receiver.recv() {
            Ok(resp) => (resp.message(), resp, true),
            Err(_) => (
                ResponseMessage::default().message(),
                ResponseMessage::default(),
                false,
            ),
        }
    }

    pub fn start_request_worker(
        &mut self,
        req: &RequestMessage,
        req_id: jsonrpc::Id,
    ) -> Receiver<ResponseMessage> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.pending_requests
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(req_id.clone(), sender);
        if self.write_msg(req.message()).is_err() {
            self.pending_requests
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .remove(&req_id);
        }
        receiver
    }
}

pub fn new_lsp_client<O>(
    server_opts: O,
    on_server_request: Option<ServerRequestHandler>,
) -> (LSPClient, impl FnOnce() -> Result<(), io::Error>)
where
    O: ServerOptionsExt,
{
    let (input_reader, input_writer) = new_lsp_pipe();
    let (output_reader, output_writer) = new_lsp_pipe();
    let server_parts = server_opts.into_test_server(input_reader, output_writer);
    let server = TestServerHandle {
        init_complete: server_parts.init_complete.clone(),
    };
    let input_writer_for_close = input_writer.clone();
    let router_input_writer = input_writer.clone();
    let pending_requests = Arc::new(Mutex::new(HashMap::new()));
    let on_server_request = Arc::new(Mutex::new(on_server_request));
    let on_server_notification = Arc::new(Mutex::new(None));
    let router_pending_requests = pending_requests.clone();
    let router_on_server_request = on_server_request.clone();
    let router_on_server_notification = on_server_notification.clone();
    let server_thread = thread::spawn(move || (server_parts.run_server)());
    let router_thread = thread::spawn(move || {
        loop {
            match LSPClient::message_router_once(
                &output_reader,
                &router_input_writer,
                &router_pending_requests,
                &router_on_server_request,
                &router_on_server_notification,
            ) {
                Ok(true) => {}
                Ok(false) => return Ok(()),
                Err(err) => return Err(err),
            }
        }
    });
    let client = LSPClient {
        server,
        input_writer,
        id: 0,
        on_server_request,
        on_server_notification,
        pending_requests,
    };
    (client, move || {
        input_writer_for_close.close();
        let server_result = match server_thread.join() {
            Ok(result) => result,
            Err(_) => Err(io::Error::new(
                io::ErrorKind::Other,
                "server thread panicked",
            )),
        };
        let router_result = match router_thread.join() {
            Ok(result) => result,
            Err(_) => Err(io::Error::new(
                io::ErrorKind::Other,
                "router thread panicked",
            )),
        };
        server_result.and(router_result)
    })
}

pub fn send_request<Params, Resp, Info, ParamsArg>(
    client: &mut LSPClient,
    info: Info,
    params: ParamsArg,
) -> (lsproto::Message, Resp, bool)
where
    Params: Clone + serde::Serialize,
    Resp: serde::de::DeserializeOwned + Default,
    Info: RequestInfoArg<Params, Resp>,
    ParamsArg: Borrow<Params>,
{
    let info = info.request_info();
    let id = client.next_id();
    let req_id = lsproto::new_id(lsproto::IntegerOrString {
        integer: Some(id),
        string: None,
    });
    let req = info.new_request_message(Some(req_id.clone()), params.borrow().clone());
    let (msg, resp, ok) = client.send_request_worker(&req, req_id);
    if !ok {
        return (msg, Resp::default(), false);
    }
    if resp.error.is_some() {
        return (msg, Resp::default(), false);
    }
    match info.unmarshal_result(resp.result) {
        Ok(result) => (msg, result, true),
        Err(_) => (msg, Resp::default(), false),
    }
}

pub fn send_request_async<Params, Resp, Info, ParamsArg>(
    client: &mut LSPClient,
    info: Info,
    params: ParamsArg,
) -> impl FnOnce(&mut LSPClient) -> (lsproto::Message, Resp, bool) + use<Params, Resp, Info, ParamsArg>
where
    Params: Clone + serde::Serialize,
    Resp: serde::de::DeserializeOwned + Default,
    Info: RequestInfoArg<Params, Resp>,
    ParamsArg: Borrow<Params>,
{
    let info = info.request_info();
    let id = client.next_id();
    let req_id = lsproto::new_id(lsproto::IntegerOrString {
        integer: Some(id),
        string: None,
    });
    let req = info.new_request_message(Some(req_id.clone()), params.borrow().clone());
    let receiver = client.send_request_async(&req, req_id);
    let info = (*info).clone();
    move |client: &mut LSPClient| {
        let (msg, resp, ok) = client.wait_for_response(receiver);
        if !ok || resp.error.is_some() {
            return (msg, Resp::default(), false);
        }
        match info.unmarshal_result(resp.result) {
            Ok(result) => (msg, result, true),
            Err(_) => (msg, Resp::default(), false),
        }
    }
}

pub fn send_notification<Params, Info, ParamsArg>(client: &LSPClient, info: Info, params: ParamsArg)
where
    Params: Clone + serde::Serialize,
    Info: NotificationInfoArg<Params>,
    ParamsArg: Borrow<Params>,
{
    let info = info.notification_info();
    let notification = info.new_notification_message(params.borrow().clone());
    let _ = client.write_msg(notification.message());
}
