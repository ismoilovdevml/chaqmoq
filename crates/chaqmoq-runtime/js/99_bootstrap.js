// Bootstrap: installs the global scope every Chaqmoq program starts with.
//
// This is the extension's ESM entry point, so it runs exactly once, before any
// user module is loaded.

import { core } from "ext:core/mod.js";

import { errors } from "ext:chaqmoq/00_errors.js";
import {
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
} from "ext:chaqmoq/01_web.js";
import { console, inspect, setColorsEnabled } from "ext:chaqmoq/02_console.js";
import {
  clearImmediate,
  clearInterval,
  clearTimeout,
  setImmediate,
  setInterval,
  setTimeout,
} from "ext:chaqmoq/03_timers.js";
import * as fs from "ext:chaqmoq/04_fs.js";
import { fetch, Headers, Request, Response, serve } from "ext:chaqmoq/05_http.js";
import { crypto } from "ext:chaqmoq/06_crypto.js";
import * as process from "ext:chaqmoq/07_process.js";
import { registeredCount, runRegisteredTests, test } from "ext:chaqmoq/08_testing.js";

const { propNonEnumerable, propWritable, propReadOnly } = core;

// ---------------------------------------------------------------------------
// Web globals
// ---------------------------------------------------------------------------

// Web platform globals are writable and configurable but not enumerable, which
// is how browsers expose them -- polyfills and test doubles keep working.
core.defineGlobalProperties(globalThis, {
  AbortController: propNonEnumerable(AbortController),
  AbortSignal: propNonEnumerable(AbortSignal),
  CustomEvent: propNonEnumerable(CustomEvent),
  DOMException: propNonEnumerable(DOMException),
  Event: propNonEnumerable(Event),
  EventTarget: propNonEnumerable(EventTarget),
  Headers: propNonEnumerable(Headers),
  Request: propNonEnumerable(Request),
  Response: propNonEnumerable(Response),
  TextDecoder: propNonEnumerable(TextDecoder),
  TextEncoder: propNonEnumerable(TextEncoder),
  URL: propNonEnumerable(URL),
  URLSearchParams: propNonEnumerable(URLSearchParams),

  atob: propWritable(atob),
  btoa: propWritable(btoa),
  console: propWritable(console),
  crypto: propReadOnly(crypto),
  fetch: propWritable(fetch),
  performance: propReadOnly(performance),
  structuredClone: propWritable(structuredClone),

  clearImmediate: propWritable(clearImmediate),
  clearInterval: propWritable(clearInterval),
  clearTimeout: propWritable(clearTimeout),
  setImmediate: propWritable(setImmediate),
  setInterval: propWritable(setInterval),
  setTimeout: propWritable(setTimeout),
});

// ---------------------------------------------------------------------------
// The Chaqmoq namespace
// ---------------------------------------------------------------------------

const Chaqmoq = {
  // -- program --
  get args() {
    return process.readArgs();
  },
  get mainModule() {
    return process.readMainModule();
  },
  get pid() {
    return process.readPid();
  },
  version: {
    chaqmoq: process.build.chaqmoq,
    v8: process.build.v8,
  },
  build: process.build,
  env: process.env,
  exit: process.exit,
  cwd: process.cwd,
  chdir: process.chdir,
  hostname: process.hostname,
  osUptime: process.osUptime,
  memoryUsage: process.memoryUsage,
  stdout: process.stdout,
  stderr: process.stderr,

  // -- filesystem --
  ...fs,

  // -- network --
  serve,

  // -- tooling --
  test,
  inspect,
  errors,
};

// Deep-freezing would break `Chaqmoq.env.set`; freezing the namespace itself is
// enough to stop accidental clobbering of the API surface.
Object.freeze(Chaqmoq);
Object.freeze(Chaqmoq.version);

core.defineGlobalProperties(globalThis, {
  Chaqmoq: propReadOnly(Chaqmoq),
});

// ---------------------------------------------------------------------------
// Runtime-internal hooks
// ---------------------------------------------------------------------------

// Not part of the public API: the CLI reaches for these through
// `globalThis.__chaqmoq` and user code has no reason to.
Object.defineProperty(globalThis, "__chaqmoq", {
  value: Object.freeze({
    setColorsEnabled,
    runRegisteredTests,
    registeredCount,
    inspect,
  }),
  enumerable: false,
  configurable: false,
  writable: false,
});

// An unhandled rejection is a bug, not a warning. Returning `false` hands the
// rejection back to deno_core, which routes it through the same uncaught
// exception path as a synchronous throw -- so the CLI prints a real stack
// trace with source-mapped frames instead of a bare "(in promise)" line.
core.setUnhandledPromiseRejectionHandler((_promise, _reason) => false);
