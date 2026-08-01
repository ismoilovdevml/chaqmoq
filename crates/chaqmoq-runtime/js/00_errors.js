// Error classes for failures that originate in Rust.
//
// `deno_core` only knows how to rebuild the standard JS error types plus a
// handful of its own. Anything an op returns under a different class name has
// to be registered here, otherwise the class is lost on the way into JS.
//
// Registering them also gives scripts something to branch on:
//
//   try { await Chaqmoq.readTextFile(path) }
//   catch (error) {
//     if (error instanceof Chaqmoq.errors.NotFound) { ... }
//   }

import { core } from "ext:core/mod.js";

/** Builds an `Error` subclass whose `name` matches its class name. */
function defineError(name) {
  const cls = class extends Error {
    constructor(message) {
      super(message);
      this.name = name;
    }
  };
  // Without this the class is reported as "cls" in stack traces.
  Object.defineProperty(cls, "name", { value: name });
  core.registerErrorClass(name, cls);
  return cls;
}

// Filesystem and process failures, named after `std::io::ErrorKind`.
const NotFound = defineError("NotFound");
const PermissionDenied = defineError("PermissionDenied");
const AlreadyExists = defineError("AlreadyExists");
const NotSupported = defineError("NotSupported");
const TimedOut = defineError("TimedOut");
const WouldBlock = defineError("WouldBlock");
const BrokenPipe = defineError("BrokenPipe");

// Network failures.
const ConnectionRefused = defineError("ConnectionRefused");
const ConnectionReset = defineError("ConnectionReset");
const ConnectionAborted = defineError("ConnectionAborted");
const NotConnected = defineError("NotConnected");
const AddrInUse = defineError("AddrInUse");
const AddrNotAvailable = defineError("AddrNotAvailable");

// Web platform error names used by `crypto` and the base64 helpers.
const QuotaExceededError = defineError("QuotaExceededError");
const NotSupportedError = defineError("NotSupportedError");
const InvalidCharacterError = defineError("InvalidCharacterError");

const errors = Object.freeze({
  // `NotCapable` is what every permission check throws. deno_core registers the
  // class itself, so this is a re-export rather than a new definition.
  NotCapable: core.NotCapable,

  NotFound,
  PermissionDenied,
  AlreadyExists,
  NotSupported,
  TimedOut,
  WouldBlock,
  BrokenPipe,

  ConnectionRefused,
  ConnectionReset,
  ConnectionAborted,
  NotConnected,
  AddrInUse,
  AddrNotAvailable,

  QuotaExceededError,
  NotSupportedError,
  InvalidCharacterError,
});

export { errors };
