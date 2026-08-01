// Web platform primitives: text encoding, base64, timing, events and URLs.
// These deliberately mirror the WHATWG/W3C specs so that code written against
// browsers or Deno keeps working.

import { core } from "ext:core/mod.js";
import {
  op_chq_atob,
  op_chq_btoa,
  op_chq_now,
  op_chq_time_origin,
  op_chq_url_parse,
  op_chq_url_parse_search_params,
  op_chq_url_set,
  op_chq_url_stringify_search_params,
} from "ext:core/ops";

// ---------------------------------------------------------------------------
// Text encoding
// ---------------------------------------------------------------------------

class TextEncoder {
  get encoding() {
    return "utf-8";
  }

  encode(input = "") {
    return core.encode(String(input));
  }

  encodeInto(source, destination) {
    const encoded = core.encode(String(source));
    // Never split a multi-byte sequence: back off to the last boundary that
    // fits, which is what the spec requires.
    let written = Math.min(encoded.length, destination.length);
    while (written > 0 && (encoded[written] & 0b1100_0000) === 0b1000_0000) {
      written--;
    }
    destination.set(encoded.subarray(0, written));
    return { read: core.decode(encoded.subarray(0, written)).length, written };
  }

  get [Symbol.toStringTag]() {
    return "TextEncoder";
  }
}

class TextDecoder {
  #encoding;
  #fatal;
  #ignoreBOM;

  constructor(label = "utf-8", options = {}) {
    const encoding = String(label).toLowerCase();
    if (encoding !== "utf-8" && encoding !== "utf8" && encoding !== "unicode-1-1-utf-8") {
      throw new RangeError(
        `The encoding label provided ('${label}') is not supported. Chaqmoq decodes UTF-8 only.`,
      );
    }
    this.#encoding = "utf-8";
    this.#fatal = Boolean(options.fatal);
    this.#ignoreBOM = Boolean(options.ignoreBOM);
  }

  get encoding() {
    return this.#encoding;
  }
  get fatal() {
    return this.#fatal;
  }
  get ignoreBOM() {
    return this.#ignoreBOM;
  }

  decode(input) {
    if (input === undefined) return "";
    let bytes;
    if (input instanceof ArrayBuffer) {
      bytes = new Uint8Array(input);
    } else if (ArrayBuffer.isView(input)) {
      bytes = new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
    } else {
      throw new TypeError("Argument must be an ArrayBuffer or ArrayBufferView");
    }
    let text = core.decode(bytes);
    if (!this.#ignoreBOM && text.charCodeAt(0) === 0xfeff) {
      text = text.slice(1);
    }
    return text;
  }

  get [Symbol.toStringTag]() {
    return "TextDecoder";
  }
}

// ---------------------------------------------------------------------------
// Base64
// ---------------------------------------------------------------------------

function btoa(data) {
  if (arguments.length === 0) {
    throw new TypeError("btoa requires 1 argument, but only 0 were passed");
  }
  return op_chq_btoa(String(data));
}

function atob(data) {
  if (arguments.length === 0) {
    throw new TypeError("atob requires 1 argument, but only 0 were passed");
  }
  return op_chq_atob(String(data));
}

// ---------------------------------------------------------------------------
// performance
// ---------------------------------------------------------------------------

const marks = new Map();

const performance = {
  now() {
    return op_chq_now();
  },
  get timeOrigin() {
    return op_chq_time_origin();
  },
  mark(name) {
    const entry = { name: String(name), entryType: "mark", startTime: op_chq_now(), duration: 0 };
    marks.set(entry.name, entry);
    return entry;
  },
  measure(name, startMark, endMark) {
    const start = startMark ? marks.get(String(startMark))?.startTime ?? 0 : 0;
    const end = endMark ? marks.get(String(endMark))?.startTime ?? op_chq_now() : op_chq_now();
    return {
      name: String(name),
      entryType: "measure",
      startTime: start,
      duration: end - start,
    };
  },
  clearMarks(name) {
    if (name === undefined) marks.clear();
    else marks.delete(String(name));
  },
  toJSON() {
    return { timeOrigin: op_chq_time_origin() };
  },
};

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

class Event {
  #type;
  #bubbles;
  #cancelable;
  #defaultPrevented = false;
  #propagationStopped = false;

  constructor(type, options = {}) {
    if (arguments.length === 0) {
      throw new TypeError("Event requires at least 1 argument");
    }
    this.#type = String(type);
    this.#bubbles = Boolean(options.bubbles);
    this.#cancelable = Boolean(options.cancelable);
    this.target = null;
    this.currentTarget = null;
  }

  get type() {
    return this.#type;
  }
  get bubbles() {
    return this.#bubbles;
  }
  get cancelable() {
    return this.#cancelable;
  }
  get defaultPrevented() {
    return this.#defaultPrevented;
  }

  preventDefault() {
    if (this.#cancelable) this.#defaultPrevented = true;
  }
  stopPropagation() {
    this.#propagationStopped = true;
  }
  stopImmediatePropagation() {
    this.#propagationStopped = true;
  }

  get [Symbol.toStringTag]() {
    return "Event";
  }
}

class CustomEvent extends Event {
  #detail;
  constructor(type, options = {}) {
    super(type, options);
    this.#detail = options.detail ?? null;
  }
  get detail() {
    return this.#detail;
  }
}

class EventTarget {
  #listeners = new Map();

  addEventListener(type, listener, options = {}) {
    if (listener === null || listener === undefined) return;
    const key = String(type);
    if (!this.#listeners.has(key)) this.#listeners.set(key, []);
    const bucket = this.#listeners.get(key);
    // The spec de-duplicates on (callback, capture); `once` is the only option
    // that changes behaviour here.
    if (bucket.some((entry) => entry.listener === listener)) return;
    bucket.push({ listener, once: Boolean(options.once) });
  }

  removeEventListener(type, listener) {
    const bucket = this.#listeners.get(String(type));
    if (!bucket) return;
    const index = bucket.findIndex((entry) => entry.listener === listener);
    if (index !== -1) bucket.splice(index, 1);
  }

  dispatchEvent(event) {
    const bucket = this.#listeners.get(event.type);
    if (!bucket || bucket.length === 0) return true;
    event.target = this;
    event.currentTarget = this;
    // Copy first: a listener may add or remove listeners while we iterate.
    for (const entry of [...bucket]) {
      if (entry.once) this.removeEventListener(event.type, entry.listener);
      try {
        if (typeof entry.listener === "function") {
          entry.listener.call(this, event);
        } else if (typeof entry.listener?.handleEvent === "function") {
          entry.listener.handleEvent(event);
        }
      } catch (error) {
        // A throwing listener must not stop the remaining listeners.
        core.reportUnhandledException(error);
      }
    }
    return !event.defaultPrevented;
  }

  get [Symbol.toStringTag]() {
    return "EventTarget";
  }
}

// A module-private key keeps `abort()` off the public `AbortSignal` surface.
// Declared before the class because the computed method name is evaluated
// when the class body is defined.
const kAbort = Symbol("[[abort]]");

class AbortSignal extends EventTarget {
  #aborted = false;
  #reason = undefined;

  static abort(reason) {
    const signal = new AbortSignal();
    signal[kAbort](reason ?? new DOMException("This operation was aborted", "AbortError"));
    return signal;
  }

  static timeout(milliseconds) {
    const signal = new AbortSignal();
    core.createSystemTimer(() => {
      signal[kAbort](new DOMException("Signal timed out", "TimeoutError"));
    }, Number(milliseconds), false);
    return signal;
  }

  get aborted() {
    return this.#aborted;
  }
  get reason() {
    return this.#reason;
  }

  throwIfAborted() {
    if (this.#aborted) throw this.#reason;
  }

  [kAbort](reason) {
    if (this.#aborted) return;
    this.#aborted = true;
    this.#reason = reason;
    if (typeof this.onabort === "function") this.onabort(new Event("abort"));
    this.dispatchEvent(new Event("abort"));
  }

  get [Symbol.toStringTag]() {
    return "AbortSignal";
  }
}

class AbortController {
  #signal = new AbortSignal();

  get signal() {
    return this.#signal;
  }

  abort(reason) {
    this.#signal[kAbort](
      reason ?? new DOMException("This operation was aborted", "AbortError"),
    );
  }

  get [Symbol.toStringTag]() {
    return "AbortController";
  }
}

class DOMException extends Error {
  #name;
  constructor(message = "", name = "Error") {
    super(message);
    this.#name = name;
  }
  get name() {
    return this.#name;
  }
  get [Symbol.toStringTag]() {
    return "DOMException";
  }
}

// ---------------------------------------------------------------------------
// URL / URLSearchParams
// ---------------------------------------------------------------------------

const kUpdateParams = Symbol("[[updateParams]]");

class URLSearchParams {
  #pairs = [];
  /** Set when this instance belongs to a `URL`, so edits propagate back. */
  #owner = null;

  constructor(init = "") {
    if (typeof init === "string") {
      this.#pairs = op_chq_url_parse_search_params(init);
    } else if (Array.isArray(init)) {
      this.#pairs = init.map(([key, value]) => [String(key), String(value)]);
    } else if (init instanceof URLSearchParams) {
      this.#pairs = [...init.#pairs];
    } else if (init && typeof init === "object") {
      this.#pairs = Object.entries(init).map(([key, value]) => [key, String(value)]);
    }
  }

  [kUpdateParams](owner) {
    this.#owner = owner;
  }

  #sync() {
    if (this.#owner) this.#owner[kUpdateParams](this.toString());
  }

  append(name, value) {
    this.#pairs.push([String(name), String(value)]);
    this.#sync();
  }

  delete(name) {
    const key = String(name);
    this.#pairs = this.#pairs.filter(([existing]) => existing !== key);
    this.#sync();
  }

  get(name) {
    const key = String(name);
    const found = this.#pairs.find(([existing]) => existing === key);
    return found ? found[1] : null;
  }

  getAll(name) {
    const key = String(name);
    return this.#pairs.filter(([existing]) => existing === key).map(([, value]) => value);
  }

  has(name) {
    const key = String(name);
    return this.#pairs.some(([existing]) => existing === key);
  }

  set(name, value) {
    const key = String(name);
    const index = this.#pairs.findIndex(([existing]) => existing === key);
    if (index === -1) {
      this.#pairs.push([key, String(value)]);
    } else {
      this.#pairs[index] = [key, String(value)];
      this.#pairs = this.#pairs.filter((pair, i) => i <= index || pair[0] !== key);
    }
    this.#sync();
  }

  sort() {
    this.#pairs.sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
    this.#sync();
  }

  forEach(callback, thisArg) {
    for (const [key, value] of this.#pairs) callback.call(thisArg, value, key, this);
  }

  *entries() {
    yield* this.#pairs.map(([key, value]) => [key, value]);
  }
  *keys() {
    for (const [key] of this.#pairs) yield key;
  }
  *values() {
    for (const [, value] of this.#pairs) yield value;
  }
  [Symbol.iterator]() {
    return this.entries();
  }

  get size() {
    return this.#pairs.length;
  }

  toString() {
    return op_chq_url_stringify_search_params(this.#pairs);
  }

  get [Symbol.toStringTag]() {
    return "URLSearchParams";
  }
}

class URL {
  #parts;
  #searchParams = null;

  constructor(url, base) {
    this.#parts = op_chq_url_parse(String(url), base === undefined ? null : String(base));
  }

  /** Called by an owned `URLSearchParams` when the query string changes. */
  [kUpdateParams](search) {
    this.#parts = op_chq_url_set(this.#parts.href, "search", search);
  }

  #set(property, value) {
    this.#parts = op_chq_url_set(this.#parts.href, property, String(value));
    // The cached search params object is now stale.
    this.#searchParams = null;
  }

  get href() {
    return this.#parts.href;
  }
  set href(value) {
    this.#parts = op_chq_url_parse(String(value), null);
    this.#searchParams = null;
  }

  get origin() {
    return this.#parts.origin;
  }

  get protocol() {
    return this.#parts.protocol;
  }
  set protocol(value) {
    this.#set("protocol", value);
  }

  get username() {
    return this.#parts.username;
  }
  set username(value) {
    this.#set("username", value);
  }

  get password() {
    return this.#parts.password;
  }
  set password(value) {
    this.#set("password", value);
  }

  get host() {
    return this.#parts.host;
  }
  set host(value) {
    const [hostname, port] = String(value).split(":");
    this.#set("hostname", hostname);
    if (port !== undefined) this.#set("port", port);
  }

  get hostname() {
    return this.#parts.hostname;
  }
  set hostname(value) {
    this.#set("hostname", value);
  }

  get port() {
    return this.#parts.port;
  }
  set port(value) {
    this.#set("port", value);
  }

  get pathname() {
    return this.#parts.pathname;
  }
  set pathname(value) {
    this.#set("pathname", value);
  }

  get search() {
    return this.#parts.search;
  }
  set search(value) {
    this.#set("search", value);
  }

  get hash() {
    return this.#parts.hash;
  }
  set hash(value) {
    this.#set("hash", value);
  }

  get searchParams() {
    if (this.#searchParams === null) {
      this.#searchParams = new URLSearchParams(this.#parts.search);
      this.#searchParams[kUpdateParams](this);
    }
    return this.#searchParams;
  }

  toString() {
    return this.#parts.href;
  }
  toJSON() {
    return this.#parts.href;
  }

  static canParse(url, base) {
    try {
      op_chq_url_parse(String(url), base === undefined ? null : String(base));
      return true;
    } catch {
      return false;
    }
  }

  get [Symbol.toStringTag]() {
    return "URL";
  }
}

function structuredClone(value, options) {
  return core.structuredClone(value, options?.transfer);
}

export {
  AbortController,
  AbortSignal,
  atob,
  btoa,
  CustomEvent,
  DOMException,
  Event,
  EventTarget,
  performance,
  structuredClone,
  TextDecoder,
  TextEncoder,
  URL,
  URLSearchParams,
};
