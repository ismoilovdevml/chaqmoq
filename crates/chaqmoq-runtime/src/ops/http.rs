//! HTTP client (`fetch`) and server (`Chaqmoq.serve`).
//!
//! Both sides are deliberately buffered: a request or response body is fully
//! materialised before it crosses the JS boundary. That keeps the op surface
//! small and predictable; streaming bodies are the natural next step and are
//! tracked in the roadmap rather than half-implemented here.

use std::cell::RefCell;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::rc::Rc;

use bytes::Bytes;
use deno_core::op2;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_error::JsErrorBox;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::Mutex;
use tokio::sync::Notify;

use crate::permissions::Permissions;

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

/// Everything the HTTP ops keep between calls, parked in `OpState`.
#[derive(Default)]
pub struct HttpState {
    client: Option<reqwest::Client>,
    next_id: u32,
    servers: HashMap<u32, ServerEntry>,
    /// Request id -> the channel hyper is waiting on for a response.
    pending: HashMap<u32, PendingRequest>,
    /// Fetch id -> the already downloaded response body.
    fetch_bodies: HashMap<u32, Vec<u8>>,
}

impl HttpState {
    fn next_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    /// One shared client means connection pooling and DNS caching across all
    /// `fetch` calls in a program.
    fn client(&mut self) -> Result<reqwest::Client, JsErrorBox> {
        if let Some(client) = &self.client {
            return Ok(client.clone());
        }
        let client = reqwest::Client::builder()
            .user_agent(format!("Chaqmoq/{}", crate::VERSION))
            .build()
            .map_err(|e| JsErrorBox::generic(format!("could not create HTTP client: {e}")))?;
        self.client = Some(client.clone());
        Ok(client)
    }
}

struct ServerEntry {
    /// Wrapped in an async mutex so that concurrent `serve_next` calls queue
    /// instead of panicking on a re-entrant borrow.
    receiver: Rc<Mutex<mpsc::UnboundedReceiver<ServerRequest>>>,
    addr: SocketAddr,
    /// Wakes a parked `serve_next`. The request channel alone is not enough:
    /// keep-alive connections hold sender clones, so the channel can stay open
    /// long after the server was told to stop.
    closed: Rc<Notify>,
    /// Dropping this sender tells the accept loop to stop.
    _shutdown: oneshot::Sender<()>,
}

struct ServerRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    responder: oneshot::Sender<ResponsePayload>,
}

struct PendingRequest {
    body: Vec<u8>,
    responder: oneshot::Sender<ResponsePayload>,
}

struct ResponsePayload {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

// ---------------------------------------------------------------------------
// fetch
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResponse {
    pub body_id: u32,
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub url: String,
    pub redirected: bool,
}

#[op2]
#[serde]
pub async fn op_chq_fetch(
    state: Rc<RefCell<OpState>>,
    #[string] method: String,
    #[string] url: String,
    #[serde] headers: Vec<(String, String)>,
    #[buffer] body: Option<JsBuffer>,
) -> Result<FetchResponse, JsErrorBox> {
    let parsed = url::Url::parse(&url)
        .map_err(|e| JsErrorBox::type_error(format!("Invalid URL: {url} ({e})")))?;

    let client = {
        let mut op_state = state.borrow_mut();
        op_state.borrow::<Permissions>().check_net_url(&parsed)?;
        op_state.borrow_mut::<HttpState>().client()?
    };

    let method = reqwest::Method::from_bytes(method.as_bytes())
        .map_err(|_| JsErrorBox::type_error(format!("Invalid HTTP method: {method}")))?;

    let mut request = client.request(method, parsed.clone());
    for (name, value) in headers {
        request = request.header(name, value);
    }
    if let Some(body) = body {
        request = request.body(body.to_vec());
    }

    let response = request
        .send()
        .await
        .map_err(|e| JsErrorBox::new("TypeError", format!("error sending request: {e}")))?;

    let status = response.status();
    let final_url = response.url().to_string();
    let redirected = final_url != parsed.to_string();
    let response_headers = response
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let bytes = response
        .bytes()
        .await
        .map_err(|e| JsErrorBox::new("TypeError", format!("error reading response body: {e}")))?;

    let body_id = {
        let mut op_state = state.borrow_mut();
        let http = op_state.borrow_mut::<HttpState>();
        let id = http.next_id();
        http.fetch_bodies.insert(id, bytes.to_vec());
        id
    };

    Ok(FetchResponse {
        body_id,
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or("").to_string(),
        headers: response_headers,
        url: final_url,
        redirected,
    })
}

/// Hands the downloaded body to JS exactly once, then frees it.
#[op2]
#[buffer]
pub fn op_chq_fetch_take_body(state: &mut OpState, #[smi] body_id: u32) -> Vec<u8> {
    state
        .borrow_mut::<HttpState>()
        .fetch_bodies
        .remove(&body_id)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Chaqmoq.serve
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenInfo {
    pub server_id: u32,
    pub hostname: String,
    pub port: u16,
}

#[op2]
#[serde]
pub fn op_chq_serve_listen(
    state: &mut OpState,
    #[string] hostname: String,
    #[smi] port: u16,
) -> Result<ListenInfo, JsErrorBox> {
    state
        .borrow::<Permissions>()
        .check_net(&hostname, Some(port))?;

    // Bind synchronously so `Chaqmoq.serve` can report the real port (which
    // matters when the caller asked for port 0).
    let std_listener = std::net::TcpListener::bind((hostname.as_str(), port))
        .map_err(|e| super::io_error(e, &format!("{hostname}:{port}")))?;
    std_listener
        .set_nonblocking(true)
        .map_err(|e| super::io_error(e, "set_nonblocking"))?;
    let addr = std_listener
        .local_addr()
        .map_err(|e| super::io_error(e, "local_addr"))?;
    let listener = tokio::net::TcpListener::from_std(std_listener)
        .map_err(|e| super::io_error(e, "from_std"))?;

    let (request_tx, request_rx) = mpsc::unbounded_channel::<ServerRequest>();
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    tokio::task::spawn(async move {
        loop {
            let (stream, _peer) = tokio::select! {
                accepted = listener.accept() => match accepted {
                    Ok(pair) => pair,
                    // A single failed accept (e.g. EMFILE) must not kill the
                    // whole server.
                    Err(_) => continue,
                },
                _ = &mut shutdown_rx => break,
            };

            let request_tx = request_tx.clone();
            tokio::task::spawn(async move {
                let service = service_fn(move |req: hyper::Request<Incoming>| {
                    let request_tx = request_tx.clone();
                    async move { dispatch(req, request_tx).await }
                });
                let _ = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                    .serve_connection(TokioIo::new(stream), service)
                    .await;
            });
        }
    });

    let http = state.borrow_mut::<HttpState>();
    let server_id = http.next_id();
    http.servers.insert(
        server_id,
        ServerEntry {
            receiver: Rc::new(Mutex::new(request_rx)),
            addr,
            closed: Rc::new(Notify::new()),
            _shutdown: shutdown_tx,
        },
    );

    Ok(ListenInfo {
        server_id,
        hostname: addr.ip().to_string(),
        port: addr.port(),
    })
}

/// Converts one hyper request into a `ServerRequest`, then waits for JS to
/// produce the matching response.
async fn dispatch(
    req: hyper::Request<Incoming>,
    request_tx: mpsc::UnboundedSender<ServerRequest>,
) -> Result<hyper::Response<Full<Bytes>>, std::convert::Infallible> {
    let method = req.method().to_string();
    let headers: Vec<(String, String)> = req
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let authority = req
        .headers()
        .get(hyper::header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(str::to_string)
        .or_else(|| req.uri().authority().map(|a| a.to_string()))
        .unwrap_or_else(|| "localhost".to_string());
    let path = req
        .uri()
        .path_and_query()
        .map(|pq| pq.to_string())
        .unwrap_or_else(|| "/".to_string());
    let url = format!("http://{authority}{path}");

    let body = match req.into_body().collect().await {
        Ok(collected) => collected.to_bytes().to_vec(),
        Err(_) => return Ok(error_response(400, "Bad Request")),
    };

    let (responder, wait) = oneshot::channel();
    let outgoing = ServerRequest {
        method,
        url,
        headers,
        body,
        responder,
    };

    if request_tx.send(outgoing).is_err() {
        // The JS side stopped reading — the runtime is shutting down.
        return Ok(error_response(503, "Service Unavailable"));
    }

    match wait.await {
        Ok(payload) => {
            let mut builder = hyper::Response::builder().status(payload.status);
            for (name, value) in &payload.headers {
                builder = builder.header(name, value);
            }
            Ok(builder
                .body(Full::new(Bytes::from(payload.body)))
                .unwrap_or_else(|_| error_response(500, "Internal Server Error")))
        }
        // The handler threw and never responded.
        Err(_) => Ok(error_response(500, "Internal Server Error")),
    }
}

fn error_response(status: u16, message: &str) -> hyper::Response<Full<Bytes>> {
    hyper::Response::builder()
        .status(status)
        .header("content-type", "text/plain; charset=utf-8")
        .body(Full::new(Bytes::from(message.to_string())))
        .expect("static response is always valid")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingRequest {
    pub request_id: u32,
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub has_body: bool,
}

#[op2]
#[serde]
pub async fn op_chq_serve_next(
    state: Rc<RefCell<OpState>>,
    #[smi] server_id: u32,
) -> Result<Option<IncomingRequest>, JsErrorBox> {
    let (receiver, closed) = {
        let op_state = state.borrow();
        let http = op_state.borrow::<HttpState>();
        match http.servers.get(&server_id) {
            Some(entry) => (entry.receiver.clone(), entry.closed.clone()),
            None => return Ok(None),
        }
    };

    let incoming = {
        let mut guard = receiver.lock().await;
        tokio::select! {
            message = guard.recv() => message,
            // `None` is how JS learns the accept loop is finished.
            _ = closed.notified() => None,
        }
    };

    let Some(incoming) = incoming else {
        return Ok(None);
    };

    let mut op_state = state.borrow_mut();
    let http = op_state.borrow_mut::<HttpState>();
    let request_id = http.next_id();
    let has_body = !incoming.body.is_empty();
    http.pending.insert(
        request_id,
        PendingRequest {
            body: incoming.body,
            responder: incoming.responder,
        },
    );

    Ok(Some(IncomingRequest {
        request_id,
        method: incoming.method,
        url: incoming.url,
        headers: incoming.headers,
        has_body,
    }))
}

#[op2]
#[buffer]
pub fn op_chq_serve_read_body(state: &mut OpState, #[smi] request_id: u32) -> Vec<u8> {
    state
        .borrow_mut::<HttpState>()
        .pending
        .get_mut(&request_id)
        .map(|pending| std::mem::take(&mut pending.body))
        .unwrap_or_default()
}

#[op2]
pub fn op_chq_serve_respond(
    state: &mut OpState,
    #[smi] request_id: u32,
    #[smi] status: u16,
    #[serde] headers: Vec<(String, String)>,
    #[buffer] body: &[u8],
) -> Result<(), JsErrorBox> {
    let pending = state
        .borrow_mut::<HttpState>()
        .pending
        .remove(&request_id)
        .ok_or_else(|| {
            JsErrorBox::generic(format!("request {request_id} was already responded to"))
        })?;

    // A send failure just means the client hung up; that is not a script error.
    let _ = pending.responder.send(ResponsePayload {
        status,
        headers,
        body: body.to_vec(),
    });
    Ok(())
}

#[op2(fast)]
pub fn op_chq_serve_close(state: &mut OpState, #[smi] server_id: u32) {
    // Wake any parked `serve_next` first, then drop the entry: dropping it also
    // drops the shutdown sender, which stops the accept loop and releases the
    // listening socket.
    if let Some(entry) = state.borrow_mut::<HttpState>().servers.remove(&server_id) {
        entry.closed.notify_waiters();
    }
}

#[op2]
#[serde]
pub fn op_chq_serve_addr(state: &mut OpState, #[smi] server_id: u32) -> Option<(String, u16)> {
    state
        .borrow::<HttpState>()
        .servers
        .get(&server_id)
        .map(|entry| (entry.addr.ip().to_string(), entry.addr.port()))
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct ServeOptions {
    pub hostname: Option<String>,
    pub port: Option<u16>,
}
