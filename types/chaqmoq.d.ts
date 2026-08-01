// Type definitions for the Chaqmoq runtime.
//
// Point your editor at this file to get completion and type checking for the
// `Chaqmoq` namespace:
//
//   /// <reference types="./types/chaqmoq.d.ts" />
//
// or add it to `compilerOptions.types` in tsconfig.json.

declare namespace Chaqmoq {
  // -------------------------------------------------------------------------
  // Program
  // -------------------------------------------------------------------------

  /** Arguments passed after the module path on the command line. */
  const args: string[];

  /** URL of the module Chaqmoq was asked to run. */
  const mainModule: string;

  /** Process id of the running program. */
  const pid: number;

  const version: {
    readonly chaqmoq: string;
    readonly v8: string;
  };

  const build: {
    readonly target: string;
    readonly arch: string;
    readonly os: string;
    readonly family: string;
    readonly v8: string;
    readonly chaqmoq: string;
    readonly rustc: string;
  };

  /**
   * Environment variables. Every accessor requires `--allow-env`; a scoped
   * `--allow-env=A,B` limits access to the named variables.
   */
  const env: {
    get(key: string): string | null;
    set(key: string, value: string): void;
    delete(key: string): void;
    has(key: string): boolean;
    toObject(): Record<string, string>;
  };

  /** Exits immediately with the given status code. */
  function exit(code?: number): never;

  /** Current working directory. Requires `--allow-read`. */
  function cwd(): string;

  /** Changes the working directory. Requires `--allow-read`. */
  function chdir(path: string | URL): void;

  /** Host name of the machine. Requires `--allow-sys`. */
  function hostname(): string;

  /** Seconds since the runtime started. */
  function osUptime(): number;

  function memoryUsage(): {
    rss: number;
    heapTotal: number;
    heapUsed: number;
    external: number;
  };

  const stdout: Writer;
  const stderr: Writer;

  interface Writer {
    write(data: string | Uint8Array): Promise<number>;
    writeSync(data: string | Uint8Array): number;
  }

  // -------------------------------------------------------------------------
  // Filesystem
  // -------------------------------------------------------------------------

  interface FileInfo {
    isFile: boolean;
    isDirectory: boolean;
    isSymlink: boolean;
    size: number;
    /** Milliseconds since the Unix epoch, or `null` if unavailable. */
    mtime: number | null;
    atime: number | null;
    birthtime: number | null;
    mode: number;
  }

  interface DirEntry {
    name: string;
    isFile: boolean;
    isDirectory: boolean;
    isSymlink: boolean;
  }

  interface WriteFileOptions {
    /** Append instead of truncating. Defaults to `false`. */
    append?: boolean;
  }

  interface MkdirOptions {
    /** Create missing parent directories. Defaults to `false`. */
    recursive?: boolean;
  }

  interface RemoveOptions {
    /** Remove directories and their contents. Defaults to `false`. */
    recursive?: boolean;
  }

  type PathLike = string | URL;

  /** Requires `--allow-read`. */
  function readFile(path: PathLike): Promise<Uint8Array>;
  function readFileSync(path: PathLike): Uint8Array;
  function readTextFile(path: PathLike): Promise<string>;
  function readTextFileSync(path: PathLike): string;

  /** Requires `--allow-write`. */
  function writeFile(
    path: PathLike,
    data: Uint8Array | ArrayBuffer | string,
    options?: WriteFileOptions,
  ): Promise<void>;
  function writeFileSync(
    path: PathLike,
    data: Uint8Array | ArrayBuffer | string,
    options?: WriteFileOptions,
  ): void;
  function writeTextFile(
    path: PathLike,
    text: string,
    options?: WriteFileOptions,
  ): Promise<void>;
  function writeTextFileSync(
    path: PathLike,
    text: string,
    options?: WriteFileOptions,
  ): void;

  /** Requires `--allow-read`. Entries are sorted by name. */
  function readDir(path: PathLike): Promise<DirEntry[]>;
  function readDirSync(path: PathLike): DirEntry[];

  /** Requires `--allow-write`. */
  function mkdir(path: PathLike, options?: MkdirOptions): Promise<void>;
  function mkdirSync(path: PathLike, options?: MkdirOptions): void;
  function remove(path: PathLike, options?: RemoveOptions): Promise<void>;
  function removeSync(path: PathLike, options?: RemoveOptions): void;
  function rename(from: PathLike, to: PathLike): Promise<void>;

  /** Requires `--allow-read` on the source and `--allow-write` on the target. */
  function copyFile(from: PathLike, to: PathLike): Promise<void>;

  /** Requires `--allow-read`. */
  function stat(path: PathLike): Promise<FileInfo>;
  function statSync(path: PathLike): FileInfo;
  /** Like `stat`, but does not follow the final symlink. */
  function lstat(path: PathLike): Promise<FileInfo>;
  function lstatSync(path: PathLike): FileInfo;
  function exists(path: PathLike): Promise<boolean>;
  function existsSync(path: PathLike): boolean;
  function realPath(path: PathLike): Promise<string>;
  function realPathSync(path: PathLike): string;

  /** Requires `--allow-write` on the system temp directory. */
  function makeTempDir(options?: { prefix?: string }): Promise<string>;
  function makeTempDirSync(options?: { prefix?: string }): string;

  // -------------------------------------------------------------------------
  // HTTP server
  // -------------------------------------------------------------------------

  interface NetAddr {
    hostname: string;
    port: number;
    transport: "tcp";
  }

  interface ServeHandlerInfo {
    remoteAddr: { transport: "tcp" };
  }

  type ServeHandler = (
    request: Request,
    info: ServeHandlerInfo,
  ) => Response | Promise<Response>;

  interface ServeOptions {
    /** Defaults to 8000. Use 0 to let the OS pick a free port. */
    port?: number;
    /** Defaults to "0.0.0.0". */
    hostname?: string;
    handler?: ServeHandler;
    /** Replaces the default "Listening on ..." message. */
    onListen?: (addr: NetAddr) => void;
    /** Called when the handler throws; should return a fallback Response. */
    onError?: (error: unknown) => Response | Promise<Response>;
  }

  interface Server {
    readonly addr: NetAddr;
    /** Resolves once the server has stopped accepting requests. */
    readonly finished: Promise<void>;
    readonly listening: boolean;
    shutdown(): void;
  }

  /**
   * Starts an HTTP server. Requires `--allow-net`.
   *
   * ```ts
   * Chaqmoq.serve({ port: 8000 }, () => new Response("hello"));
   * ```
   */
  function serve(handler: ServeHandler): Server;
  function serve(options: ServeOptions, handler?: ServeHandler): Server;

  // -------------------------------------------------------------------------
  // Testing
  // -------------------------------------------------------------------------

  interface TestDefinition {
    name: string;
    fn?: () => void | Promise<void>;
    /** Report as ignored without running. */
    ignore?: boolean;
    /** Run only tests marked `only`. */
    only?: boolean;
  }

  /** Registers a test for `chaqmoq test` to run. */
  function test(name: string, fn: () => void | Promise<void>): void;
  function test(definition: TestDefinition, fn?: () => void | Promise<void>): void;
  function test(fn: () => void | Promise<void>): void;

  // -------------------------------------------------------------------------
  // Utilities
  // -------------------------------------------------------------------------

  /** Renders any value the way `console.log` would. */
  function inspect(value: unknown, options?: { depth?: number }): string;

  // -------------------------------------------------------------------------
  // Errors
  // -------------------------------------------------------------------------

  /**
   * Error classes thrown by the runtime, so failures can be matched by kind:
   *
   * ```ts
   * try {
   *   await Chaqmoq.readTextFile(path);
   * } catch (error) {
   *   if (error instanceof Chaqmoq.errors.NotFound) return null;
   *   throw error;
   * }
   * ```
   */
  namespace errors {
    /** A permission check failed. Grant the matching `--allow-*` flag. */
    class NotCapable extends Error {}

    class NotFound extends Error {}
    class PermissionDenied extends Error {}
    class AlreadyExists extends Error {}
    class NotSupported extends Error {}
    class TimedOut extends Error {}
    class WouldBlock extends Error {}
    class BrokenPipe extends Error {}

    class ConnectionRefused extends Error {}
    class ConnectionReset extends Error {}
    class ConnectionAborted extends Error {}
    class NotConnected extends Error {}
    class AddrInUse extends Error {}
    class AddrNotAvailable extends Error {}

    class QuotaExceededError extends Error {}
    class NotSupportedError extends Error {}
    class InvalidCharacterError extends Error {}
  }
}
