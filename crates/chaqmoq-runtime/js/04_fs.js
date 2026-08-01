// Filesystem APIs on the `Chaqmoq` namespace.
//
// Every function has an async form and a `*Sync` form. Async is the default you
// should reach for; the sync forms exist for start-up configuration reading and
// short scripts where blocking the isolate is fine.

import { TextEncoder, URL } from "ext:chaqmoq/01_web.js";
import {
  op_chq_copy_file,
  op_chq_exists_sync,
  op_chq_make_temp_dir,
  op_chq_mkdir,
  op_chq_mkdir_sync,
  op_chq_read_dir,
  op_chq_read_dir_sync,
  op_chq_read_file,
  op_chq_read_file_sync,
  op_chq_read_text_file,
  op_chq_read_text_file_sync,
  op_chq_real_path_sync,
  op_chq_remove,
  op_chq_remove_sync,
  op_chq_rename,
  op_chq_stat,
  op_chq_stat_sync,
  op_chq_write_file,
  op_chq_write_file_sync,
} from "ext:core/ops";

/** Accepts a string, a `URL`, or anything stringifiable, like Deno's APIs. */
function toPath(path) {
  if (typeof path === "string") return path;
  if (path instanceof URL) {
    if (path.protocol !== "file:") {
      throw new TypeError(`Expected a file:// URL, got ${path.protocol}//`);
    }
    return decodeURIComponent(path.pathname);
  }
  return String(path);
}

function toBytes(data) {
  if (typeof data === "string") return new TextEncoder().encode(data);
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (ArrayBuffer.isView(data)) {
    return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  }
  throw new TypeError("data must be a string, ArrayBuffer or ArrayBufferView");
}

// ---------------------------------------------------------------------------
// Read
// ---------------------------------------------------------------------------

function readFile(path) {
  return op_chq_read_file(toPath(path));
}

function readFileSync(path) {
  return op_chq_read_file_sync(toPath(path));
}

function readTextFile(path) {
  return op_chq_read_text_file(toPath(path));
}

function readTextFileSync(path) {
  return op_chq_read_text_file_sync(toPath(path));
}

// ---------------------------------------------------------------------------
// Write
// ---------------------------------------------------------------------------

function writeFile(path, data, options = {}) {
  return op_chq_write_file(toPath(path), toBytes(data), Boolean(options.append));
}

function writeFileSync(path, data, options = {}) {
  return op_chq_write_file_sync(toPath(path), toBytes(data), Boolean(options.append));
}

function writeTextFile(path, text, options = {}) {
  return op_chq_write_file(toPath(path), toBytes(String(text)), Boolean(options.append));
}

function writeTextFileSync(path, text, options = {}) {
  return op_chq_write_file_sync(toPath(path), toBytes(String(text)), Boolean(options.append));
}

// ---------------------------------------------------------------------------
// Directories & metadata
// ---------------------------------------------------------------------------

async function readDir(path) {
  return await op_chq_read_dir(toPath(path));
}

function readDirSync(path) {
  return op_chq_read_dir_sync(toPath(path));
}

function mkdir(path, options = {}) {
  return op_chq_mkdir(toPath(path), Boolean(options.recursive));
}

function mkdirSync(path, options = {}) {
  return op_chq_mkdir_sync(toPath(path), Boolean(options.recursive));
}

function remove(path, options = {}) {
  return op_chq_remove(toPath(path), Boolean(options.recursive));
}

function removeSync(path, options = {}) {
  return op_chq_remove_sync(toPath(path), Boolean(options.recursive));
}

function stat(path) {
  return op_chq_stat(toPath(path), false);
}

function statSync(path) {
  return op_chq_stat_sync(toPath(path), false);
}

/** Like `stat`, but does not follow the final symlink. */
function lstat(path) {
  return op_chq_stat(toPath(path), true);
}

function lstatSync(path) {
  return op_chq_stat_sync(toPath(path), true);
}

/**
 * Reports whether a path exists.
 *
 * Prefer `try { await stat(p) } catch {}` when you are about to open the file:
 * checking first is a time-of-check/time-of-use race.
 */
function exists(path) {
  return Promise.resolve(op_chq_exists_sync(toPath(path)));
}

function existsSync(path) {
  return op_chq_exists_sync(toPath(path));
}

function copyFile(from, to) {
  return op_chq_copy_file(toPath(from), toPath(to));
}

function rename(from, to) {
  return op_chq_rename(toPath(from), toPath(to));
}

function realPathSync(path) {
  return op_chq_real_path_sync(toPath(path));
}

function realPath(path) {
  return Promise.resolve(op_chq_real_path_sync(toPath(path)));
}

function makeTempDir(options = {}) {
  return Promise.resolve(op_chq_make_temp_dir(String(options.prefix ?? "chaqmoq_")));
}

function makeTempDirSync(options = {}) {
  return op_chq_make_temp_dir(String(options.prefix ?? "chaqmoq_"));
}

export {
  copyFile,
  exists,
  existsSync,
  lstat,
  lstatSync,
  makeTempDir,
  makeTempDirSync,
  mkdir,
  mkdirSync,
  readDir,
  readDirSync,
  readFile,
  readFileSync,
  readTextFile,
  readTextFileSync,
  realPath,
  realPathSync,
  remove,
  removeSync,
  rename,
  stat,
  statSync,
  writeFile,
  writeFileSync,
  writeTextFile,
  writeTextFileSync,
};
