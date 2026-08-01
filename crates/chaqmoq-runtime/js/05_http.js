// HTTP: `fetch` for outbound requests and `Chaqmoq.serve` for inbound ones,
// sharing one set of `Headers` / `Request` / `Response` classes.

import { TextDecoder, TextEncoder, URLSearchParams } from "ext:chaqmoq/01_web.js";
import { console } from "ext:chaqmoq/02_console.js";
import {
  op_chq_fetch,
  op_chq_fetch_take_body,
  op_chq_serve_addr,
  op_chq_serve_close,
  op_chq_serve_listen,
  op_chq_serve_next,
  op_chq_serve_read_body,
  op_chq_serve_respond,
} from "ext:core/ops";

// Headers that may legitimately appear more than once are joined with ", ";
// `set-cookie` is the documented exception and is kept as separate entries.
const FORBIDDEN_SINGLETON = "set-cookie";

class Headers {
  /** @type {Array<[string, string]>} lower-cased name, raw value */
  #list = [];

  constructor(init) {
    if (init instanceof Headers) {
      this.#list = [...init.#list];
    } else if (Array.isArray(init)) {
      for (const [name, value] of init) this.append(name, value);
    } else if (init && typeof init === "object") {
      for (const [name, value] of Object.entries(init)) this.append(name, value);
    }
  }

  #normalize(name) {
    const normalized = String(name).toLowerCase();
    if (!/^[!#$%&'*+\-.^_`|~0-9a-z]+$/.test(normalized)) {
      throw new TypeError(`Invalid header name: ${name}`);
    }
    return normalized;
  }

  append(name, value) {
    this.#list.push([this.#normalize(name), String(value).trim()]);
  }

  set(name, value) {
    const key = this.#normalize(name);
    this.#list = this.#list.filter(([existing]) => existing !== key);
    this.#list.push([key, String(value).trim()]);
  }

  get(name) {
    const key = this.#normalize(name);
    const matches = this.#list.filter(([existing]) => existing === key);
    if (matches.length === 0) return null;
    return matches.map(([, value]) => value).join(", ");
  }

  getSetCookie() {
    return this.#list
      .filter(([name]) => name === FORBIDDEN_SINGLETON)
      .map(([, value]) => value);
  }

  has(name) {
    const key = this.#normalize(name);
    return this.#list.some(([existing]) => existing === key);
  }

  delete(name) {
    const key = this.#normalize(name);
    this.#list = this.#list.filter(([existing]) => existing !== key);
  }

  forEach(callback, thisArg) {
    for (const [name, value] of this.entries()) callback.call(thisArg, value, name, this);
  }

  *entries() {
    // The spec iterates in sorted order with duplicates combined.
    const names = [...new Set(this.#list.map(([name]) => name))].sort();
    for (const name of names) {
      if (name === FORBIDDEN_SINGLETON) {
        for (const value of this.getSetCookie()) yield [name, value];
      } else {
        yield [name, this.get(name)];
      }
    }
  }
  *keys() {
    for (const [name] of this.entries()) yield name;
  }
  *values() {
    for (const [, value] of this.entries()) yield value;
  }
  [Symbol.iterator]() {
    return this.entries();
  }

  /** Flat pair list in the shape the Rust ops expect. */
  toPairs() {
    return this.#list.map(([name, value]) => [name, value]);
  }

  get [Symbol.toStringTag]() {
    return "Headers";
  }
}

// Runtime-internal keys. Declared before the classes that use them as computed
// method names, and never exported, so scripts cannot reach them.
const kRawBody = Symbol("[[rawBody]]");
const kLazyBody = Symbol("[[lazyBody]]");

/**
 * Shared body behaviour for `Request` and `Response`.
 *
 * Bodies are fully buffered. The source is either the bytes themselves or a
 * thunk that pulls them from Rust on first use, so a response whose body is
 * never read costs nothing to move across the boundary.
 */
class Body {
  #source;
  #bytes = null;
  #used = false;

  /** @param {Uint8Array | null | (() => Uint8Array)} source */
  constructor(source) {
    this.#source = source;
  }

  get bodyUsed() {
    return this.#used;
  }

  /** Reads the bytes without marking the body consumed (runtime-internal). */
  [kRawBody]() {
    if (this.#bytes === null) {
      this.#bytes = typeof this.#source === "function" ? this.#source() : this.#source;
    }
    return this.#bytes ?? new Uint8Array(0);
  }

  #consume() {
    if (this.#used) {
      throw new TypeError("Body has already been consumed");
    }
    this.#used = true;
    if (this.#bytes === null) {
      this.#bytes = typeof this.#source === "function" ? this.#source() : this.#source;
    }
    return this.#bytes ?? new Uint8Array(0);
  }

  async arrayBuffer() {
    const bytes = this.#consume();
    // Slice so the caller cannot observe the runtime's backing store.
    return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
  }

  async bytes() {
    return this.#consume();
  }

  async text() {
    return new TextDecoder().decode(this.#consume());
  }

  async json() {
    return JSON.parse(await this.text());
  }
}

const NULL_BODY_STATUS = new Set([101, 204, 205, 304]);

class Response extends Body {
  #status;
  #statusText;
  #headers;
  #url;
  #redirected;

  constructor(body = null, init = {}) {
    const status = init.status ?? 200;
    if (status < 200 || status > 599) {
      throw new RangeError(`Invalid response status code: ${status}`);
    }
    const headers = new Headers(init.headers);
    // `kLazyBody` is how `fetch` hands over a body that is still sitting in
    // Rust; user code always goes through `normalizeBody`.
    const bytes = init[kLazyBody] ?? normalizeBody(body, headers);
    super(bytes);

    this.#status = status;
    this.#statusText = init.statusText ?? "";
    this.#headers = headers;
    this.#url = init.url ?? "";
    this.#redirected = Boolean(init.redirected);
  }

  get status() {
    return this.#status;
  }
  get statusText() {
    return this.#statusText;
  }
  get headers() {
    return this.#headers;
  }
  get url() {
    return this.#url;
  }
  get redirected() {
    return this.#redirected;
  }
  get ok() {
    return this.#status >= 200 && this.#status < 300;
  }

  static json(data, init = {}) {
    const headers = new Headers(init.headers);
    if (!headers.has("content-type")) {
      headers.set("content-type", "application/json; charset=utf-8");
    }
    return new Response(JSON.stringify(data), { ...init, headers });
  }

  static redirect(url, status = 302) {
    if (![301, 302, 303, 307, 308].includes(status)) {
      throw new RangeError(`Invalid redirect status code: ${status}`);
    }
    return new Response(null, { status, headers: { location: String(url) } });
  }

  static error() {
    return new Response(null, { status: 500 });
  }

  get [Symbol.toStringTag]() {
    return "Response";
  }
}

class Request extends Body {
  #method;
  #url;
  #headers;

  constructor(input, init = {}) {
    const url = input instanceof Request ? input.url : String(input);
    const headers = new Headers(init.headers ?? (input instanceof Request ? input.headers : undefined));
    const bytes = normalizeBody(init.body ?? null, headers);
    super(bytes);

    this.#method = String(init.method ?? (input instanceof Request ? input.method : "GET")).toUpperCase();
    this.#url = url;
    this.#headers = headers;
  }

  get method() {
    return this.#method;
  }
  get url() {
    return this.#url;
  }
  get headers() {
    return this.#headers;
  }

  get [Symbol.toStringTag]() {
    return "Request";
  }
}

/** Converts a user-supplied body into bytes and sets a default content-type. */
function normalizeBody(body, headers) {
  if (body === null || body === undefined) return null;

  if (typeof body === "string") {
    if (!headers.has("content-type")) {
      headers.set("content-type", "text/plain;charset=UTF-8");
    }
    return new TextEncoder().encode(body);
  }
  if (body instanceof URLSearchParams) {
    if (!headers.has("content-type")) {
      headers.set("content-type", "application/x-www-form-urlencoded;charset=UTF-8");
    }
    return new TextEncoder().encode(body.toString());
  }
  if (body instanceof ArrayBuffer) return new Uint8Array(body);
  if (ArrayBuffer.isView(body)) {
    return new Uint8Array(body.buffer, body.byteOffset, body.byteLength);
  }
  throw new TypeError("Unsupported body type");
}

// ---------------------------------------------------------------------------
// fetch
// ---------------------------------------------------------------------------

async function fetch(input, init = {}) {
  const request = new Request(input, init);
  const body = request[kRawBody]();

  const result = await op_chq_fetch(
    request.method,
    request.url,
    request.headers.toPairs(),
    body.byteLength > 0 ? body : null,
  );

  // The payload stays in Rust until something actually reads it.
  return new Response(null, {
    status: result.status,
    statusText: result.statusText,
    headers: result.headers,
    url: result.url,
    redirected: result.redirected,
    [kLazyBody]: () => op_chq_fetch_take_body(result.bodyId),
  });
}

// ---------------------------------------------------------------------------
// Chaqmoq.serve
// ---------------------------------------------------------------------------

/**
 * Starts an HTTP server.
 *
 * ```js
 * Chaqmoq.serve({ port: 8000 }, (request) => new Response("hi"));
 * ```
 *
 * @returns {{addr: {hostname: string, port: number}, finished: Promise<void>, shutdown: () => void}}
 */
function serve(optionsOrHandler, maybeHandler) {
  let options = {};
  let handler = maybeHandler;

  if (typeof optionsOrHandler === "function") {
    handler = optionsOrHandler;
  } else if (optionsOrHandler && typeof optionsOrHandler === "object") {
    options = optionsOrHandler;
    handler = maybeHandler ?? options.handler;
  }

  if (typeof handler !== "function") {
    throw new TypeError("Chaqmoq.serve requires a handler function");
  }

  const hostname = options.hostname ?? "0.0.0.0";
  const port = options.port ?? 8000;

  const listener = op_chq_serve_listen(hostname, port);
  const addr = { hostname: listener.hostname, port: listener.port, transport: "tcp" };

  if (typeof options.onListen === "function") {
    options.onListen(addr);
  } else {
    const display = addr.hostname === "0.0.0.0" ? "localhost" : addr.hostname;
    console.log(`Listening on http://${display}:${addr.port}/`);
  }

  let running = true;

  const finished = (async () => {
    while (running) {
      const incoming = await op_chq_serve_next(listener.serverId);
      // `null` means the server was closed.
      if (!incoming) break;

      // Handle concurrently: a slow handler must not block the accept loop.
      void respond(incoming, handler, options);
    }
  })();

  return {
    addr,
    finished,
    shutdown() {
      running = false;
      op_chq_serve_close(listener.serverId);
    },
    get listening() {
      return op_chq_serve_addr(listener.serverId) !== null;
    },
  };
}

async function respond(incoming, handler, options) {
  const requestInit = {
    method: incoming.method,
    headers: incoming.headers,
  };
  if (incoming.hasBody) {
    requestInit.body = op_chq_serve_read_body(incoming.requestId);
  }

  let response;
  try {
    response = await handler(new Request(incoming.url, requestInit), {
      remoteAddr: { transport: "tcp" },
    });
    if (!(response instanceof Response)) {
      throw new TypeError(
        `The serve handler must return a Response, got ${typeof response}`,
      );
    }
  } catch (error) {
    if (typeof options.onError === "function") {
      try {
        response = await options.onError(error);
      } catch {
        response = new Response("Internal Server Error", { status: 500 });
      }
    } else {
      console.error("Uncaught error in serve handler:", error);
      response = new Response("Internal Server Error", { status: 500 });
    }
  }

  let body;
  try {
    body = await response.bytes();
  } catch {
    body = new Uint8Array(0);
  }

  const status = response.status;
  op_chq_serve_respond(
    incoming.requestId,
    status,
    response.headers.toPairs(),
    // 204/304 and friends must not carry a body.
    NULL_BODY_STATUS.has(status) ? new Uint8Array(0) : body,
  );
}

export { fetch, Headers, Request, Response, serve };
