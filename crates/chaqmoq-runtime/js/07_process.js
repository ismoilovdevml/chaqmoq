// Process and platform surface of the `Chaqmoq` namespace.

import { core } from "ext:core/mod.js";
import { TextDecoder } from "ext:chaqmoq/01_web.js";
import {
  op_chq_args,
  op_chq_build_info,
  op_chq_chdir,
  op_chq_cwd,
  op_chq_env_delete,
  op_chq_env_get,
  op_chq_env_set,
  op_chq_env_to_object,
  op_chq_exit,
  op_chq_hostname,
  op_chq_main_module,
  op_chq_pid,
  op_chq_uptime,
} from "ext:core/ops";

/**
 * `Chaqmoq.env` -- every accessor is permission checked in Rust, so a program
 * running without `--allow-env` throws here rather than silently seeing
 * `undefined`.
 */
const env = {
  get(key) {
    return op_chq_env_get(String(key));
  },
  set(key, value) {
    op_chq_env_set(String(key), String(value));
  },
  delete(key) {
    op_chq_env_delete(String(key));
  },
  has(key) {
    return op_chq_env_get(String(key)) !== null;
  },
  toObject() {
    return op_chq_env_to_object();
  },
  get [Symbol.toStringTag]() {
    return "Env";
  },
};

function exit(code = 0) {
  op_chq_exit(Number(code) | 0);
}

function cwd() {
  return op_chq_cwd();
}

function chdir(path) {
  op_chq_chdir(String(path));
}

function hostname() {
  return op_chq_hostname();
}

function osUptime() {
  return op_chq_uptime();
}

function memoryUsage() {
  const usage = core.memoryUsage();
  return {
    rss: usage.rss,
    heapTotal: usage.heapTotal,
    heapUsed: usage.heapUsed,
    external: usage.external,
  };
}

/** Minimal writable stdio, enough for progress output and REPL-style tools. */
function makeWriter(isError) {
  const decoder = new TextDecoder();
  return {
    write(data) {
      const text = typeof data === "string" ? data : decoder.decode(data);
      core.print(text, isError);
      return Promise.resolve(typeof data === "string" ? data.length : data.byteLength);
    },
    writeSync(data) {
      const text = typeof data === "string" ? data : decoder.decode(data);
      core.print(text, isError);
      return typeof data === "string" ? data.length : data.byteLength;
    },
  };
}

const stdout = makeWriter(false);
const stderr = makeWriter(true);

const build = op_chq_build_info();

export {
  build,
  chdir,
  cwd,
  env,
  exit,
  hostname,
  memoryUsage,
  op_chq_args as readArgs,
  op_chq_main_module as readMainModule,
  op_chq_pid as readPid,
  osUptime,
  stderr,
  stdout,
};
