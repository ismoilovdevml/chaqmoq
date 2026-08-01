// Web timers on top of `deno_core`'s native timer wheel.
//
// `core.createTimer` returns an opaque handle; the web API contract is a
// numeric id, so we keep a map between the two.

import { core } from "ext:core/mod.js";

const handles = new Map();
let nextId = 1;

function schedule(callback, delay, args, repeat) {
  if (typeof callback !== "function") {
    // The browser-compatible fallback: a string argument is evaluated. Chaqmoq
    // refuses instead of shipping an `eval` foot-gun.
    throw new TypeError(
      "The callback argument must be a function. Chaqmoq does not evaluate string timer callbacks.",
    );
  }

  const id = nextId++;
  const timeout = Number(delay) || 0;

  const handle = core.createTimer(
    (...timerArgs) => {
      // A one-shot timer is done as soon as it fires; an interval stays
      // registered so `clearInterval` keeps working.
      if (!repeat) handles.delete(id);
      callback(...timerArgs);
    },
    timeout,
    args,
    repeat,
    /* isRefed */ true,
    /* isSystem */ false,
  );

  handles.set(id, handle);
  return id;
}

function setTimeout(callback, delay = 0, ...args) {
  return schedule(callback, delay, args, false);
}

function setInterval(callback, delay = 0, ...args) {
  return schedule(callback, delay, args, true);
}

function clearTimer(id) {
  const handle = handles.get(id);
  if (handle === undefined) return;
  handles.delete(id);
  core.cancelTimer(handle);
}

/**
 * Runs `callback` after the current turn of the event loop, before any pending
 * timers. Chaqmoq maps it onto the shortest possible timer, which preserves
 * ordering relative to I/O without needing a separate queue.
 */
function setImmediate(callback, ...args) {
  return schedule(callback, 0, args, false);
}

export {
  clearTimer as clearImmediate,
  clearTimer as clearInterval,
  clearTimer as clearTimeout,
  setImmediate,
  setInterval,
  setTimeout,
};
