// `console` and the value inspector behind it.
//
// The inspector is intentionally self-contained: it walks values itself rather
// than deferring to `JSON.stringify`, so it can render Maps, Sets, class
// instances, getters, circular references and typed arrays the way a developer
// expects to see them.

import { core } from "ext:core/mod.js";
import { performance } from "ext:chaqmoq/01_web.js";

const STYLES = {
  number: "33", // yellow
  bigint: "33",
  boolean: "33",
  undefined: "90", // bright black
  null: "1", // bold
  string: "32", // green
  symbol: "32",
  date: "35", // magenta
  regexp: "31", // red
  special: "36", // cyan -- functions and classes
  key: "0",
};

let colorsEnabled = false;

/** The CLI decides once, at startup, whether stdout is a colour-capable TTY. */
function setColorsEnabled(value) {
  colorsEnabled = Boolean(value);
}

function style(text, kind) {
  const code = STYLES[kind];
  if (!colorsEnabled || code === undefined) return text;
  return `\x1b[${code}m${text}\x1b[0m`;
}

const QUOTE_ESCAPES = {
  "\n": "\\n",
  "\t": "\\t",
  "\r": "\\r",
  "\b": "\\b",
  "\f": "\\f",
  "\v": "\\v",
  "\\": "\\\\",
};

function quoteString(value) {
  // Prefer the quote character that needs the least escaping, like Node.
  const quote = !value.includes("'") ? "'" : !value.includes('"') ? '"' : "`";
  let out = "";
  for (const char of value) {
    if (char === quote) out += `\\${char}`;
    else if (QUOTE_ESCAPES[char]) out += QUOTE_ESCAPES[char];
    else if (char < " ") out += `\\x${char.charCodeAt(0).toString(16).padStart(2, "0")}`;
    else out += char;
  }
  return `${quote}${out}${quote}`;
}

/** Property keys that are valid identifiers are printed bare. */
const IDENTIFIER = /^[A-Za-z_$][A-Za-z0-9_$]*$/;

function formatKey(key) {
  if (typeof key === "symbol") return `[${style(key.toString(), "symbol")}]`;
  return IDENTIFIER.test(key) ? key : quoteString(key);
}

function className(value) {
  const name = value?.constructor?.name;
  if (!name || name === "Object") return "";
  return `${name} `;
}

/**
 * Renders `value` as a single string.
 *
 * @param {unknown} value
 * @param {{depth?: number, seen?: Set<object>, level?: number}} options
 */
function inspect(value, options = {}) {
  const depth = options.depth ?? 4;
  return format(value, depth, options.seen ?? new Set(), 0);
}

function format(value, depth, seen, level) {
  switch (typeof value) {
    case "string":
      // Top-level strings print bare; nested ones are quoted so that
      // `["a"]` is distinguishable from `[a]`.
      return level === 0 ? value : style(quoteString(value), "string");
    case "number":
      // `-0` is worth distinguishing from `0` when debugging.
      return style(Object.is(value, -0) ? "-0" : String(value), "number");
    case "bigint":
      return style(`${value}n`, "bigint");
    case "boolean":
      return style(String(value), "boolean");
    case "undefined":
      return style("undefined", "undefined");
    case "symbol":
      return style(value.toString(), "symbol");
    case "function":
      return formatFunction(value);
    case "object":
      break;
    default:
      return String(value);
  }

  if (value === null) return style("null", "null");

  if (seen.has(value)) return style("[Circular *1]", "special");
  if (depth < 0) return style(`[${value.constructor?.name ?? "Object"}]`, "special");

  if (value instanceof Date) {
    const text = Number.isNaN(value.getTime()) ? "Invalid Date" : value.toISOString();
    return style(text, "date");
  }
  if (value instanceof RegExp) return style(String(value), "regexp");
  if (value instanceof Error) return formatError(value);

  seen.add(value);
  try {
    if (Array.isArray(value)) return formatArray(value, depth, seen, level);
    if (value instanceof Map) return formatMap(value, depth, seen, level);
    if (value instanceof Set) return formatSet(value, depth, seen, level);
    if (ArrayBuffer.isView(value)) return formatTypedArray(value, depth, seen, level);
    if (value instanceof ArrayBuffer) {
      return `ArrayBuffer { byteLength: ${style(String(value.byteLength), "number")} }`;
    }
    if (value instanceof Promise) return `Promise { ${style("<pending>", "special")} }`;
    if (value instanceof WeakMap) return "WeakMap { <items unknown> }";
    if (value instanceof WeakSet) return "WeakSet { <items unknown> }";
    return formatObject(value, depth, seen, level);
  } finally {
    // Removing on the way out means a value repeated in sibling positions is
    // rendered fully both times; only true cycles get `[Circular]`.
    seen.delete(value);
  }
}

function formatFunction(value) {
  const isClass = /^class[\s{]/.test(Function.prototype.toString.call(value));
  const name = value.name;
  if (isClass) return style(name ? `[class ${name}]` : "[class (anonymous)]", "special");
  return style(name ? `[Function: ${name}]` : "[Function (anonymous)]", "special");
}

function formatError(error) {
  const stack = error.stack;
  if (typeof stack === "string" && stack.length > 0) return stack;
  return `${error.name}: ${error.message}`;
}

/** Chooses between one-line and multi-line layout based on rendered width. */
function wrap(parts, open, close, prefix, level) {
  if (parts.length === 0) return `${prefix}${open}${close}`;
  const oneLine = `${prefix}${open} ${parts.join(", ")} ${close}`;
  // 72 leaves room for the indentation that a nested value already carries.
  if (oneLine.length <= 72 && !oneLine.includes("\n")) return oneLine;

  const indent = "  ".repeat(level + 1);
  const closingIndent = "  ".repeat(level);
  return `${prefix}${open}\n${parts.map((p) => indent + p).join(",\n")}\n${closingIndent}${close}`;
}

const MAX_ENTRIES = 100;

function truncate(parts, total) {
  if (total > MAX_ENTRIES) {
    const hidden = total - MAX_ENTRIES;
    parts.push(style(`... ${hidden} more item${hidden === 1 ? "" : "s"}`, "undefined"));
  }
  return parts;
}

function formatArray(value, depth, seen, level) {
  const parts = [];
  const limit = Math.min(value.length, MAX_ENTRIES);
  for (let i = 0; i < limit; i++) {
    // Holes in a sparse array are not the same as `undefined` values.
    parts.push(i in value ? format(value[i], depth - 1, seen, level + 1) : style("<empty>", "undefined"));
  }
  truncate(parts, value.length);

  // Non-index properties tacked onto an array still deserve to be shown.
  for (const key of Object.keys(value)) {
    if (!/^\d+$/.test(key)) {
      parts.push(`${formatKey(key)}: ${format(value[key], depth - 1, seen, level + 1)}`);
    }
  }
  return wrap(parts, "[", "]", "", level);
}

function formatTypedArray(value, depth, seen, level) {
  const parts = [];
  const limit = Math.min(value.length, MAX_ENTRIES);
  for (let i = 0; i < limit; i++) {
    parts.push(format(value[i], depth - 1, seen, level + 1));
  }
  truncate(parts, value.length);
  return wrap(parts, "[", "]", `${value.constructor.name}(${value.length}) `, level);
}

function formatMap(value, depth, seen, level) {
  const parts = [];
  let count = 0;
  for (const [key, entry] of value) {
    if (count++ >= MAX_ENTRIES) break;
    parts.push(
      `${format(key, depth - 1, seen, level + 1)} => ${format(entry, depth - 1, seen, level + 1)}`,
    );
  }
  truncate(parts, value.size);
  return wrap(parts, "{", "}", `Map(${value.size}) `, level);
}

function formatSet(value, depth, seen, level) {
  const parts = [];
  let count = 0;
  for (const entry of value) {
    if (count++ >= MAX_ENTRIES) break;
    parts.push(format(entry, depth - 1, seen, level + 1));
  }
  truncate(parts, value.size);
  return wrap(parts, "{", "}", `Set(${value.size}) `, level);
}

function formatObject(value, depth, seen, level) {
  const parts = [];
  const descriptors = Object.getOwnPropertyDescriptors(value);

  for (const key of Reflect.ownKeys(descriptors)) {
    const descriptor = descriptors[key];
    if (!descriptor.enumerable) continue;

    if (descriptor.get) {
      // Never invoke a getter: it may be expensive or throw.
      parts.push(`${formatKey(key)}: ${style("[Getter]", "special")}`);
      continue;
    }
    parts.push(`${formatKey(key)}: ${format(descriptor.value, depth - 1, seen, level + 1)}`);
  }

  // A null-prototype object is a meaningfully different thing.
  const prefix = Object.getPrototypeOf(value) === null ? "[Object: null prototype] " : className(value);
  return wrap(parts, "{", "}", prefix, level);
}

function argsToString(args) {
  // `console.log("%s items", 3)` style formatting.
  if (typeof args[0] === "string" && args[0].includes("%") && args.length > 1) {
    return applyFormat(args);
  }
  return args.map((arg) => inspect(arg)).join(" ");
}

function applyFormat(args) {
  const template = args[0];
  const rest = args.slice(1);
  let index = 0;
  const formatted = template.replace(/%[sdifjoOc%]/g, (token) => {
    if (token === "%%") return "%";
    if (index >= rest.length) return token;
    const value = rest[index++];
    switch (token) {
      case "%s":
        return typeof value === "string" ? value : inspect(value, { depth: 1 });
      case "%d":
      case "%i":
        return typeof value === "bigint" ? `${value}n` : String(Math.floor(Number(value)));
      case "%f":
        return String(Number(value));
      case "%j":
        try {
          return JSON.stringify(value);
        } catch {
          return "[Circular]";
        }
      case "%o":
      case "%O":
        return inspect(value, { depth: 8 });
      case "%c":
        // CSS styling has no meaning in a terminal.
        return "";
      default:
        return token;
    }
  });
  const leftover = rest.slice(index).map((arg) => inspect(arg));
  return [formatted, ...leftover].join(" ");
}

// ---------------------------------------------------------------------------
// console
// ---------------------------------------------------------------------------

let groupIndent = "";
const counters = new Map();
const timers = new Map();

function write(text, isError) {
  const indented = groupIndent
    ? text.split("\n").map((line) => groupIndent + line).join("\n")
    : text;
  core.print(`${indented}\n`, isError);
}

function buildTable(data, columns) {
  const rows = [];
  const isArray = Array.isArray(data);
  const entries = isArray ? data.entries() : Object.entries(data);

  const valueColumns = new Set();
  let hasValues = false;

  for (const [key, value] of entries) {
    const row = { "(index)": String(key) };
    if (value !== null && typeof value === "object" && !Array.isArray(value)) {
      for (const [column, cell] of Object.entries(value)) {
        valueColumns.add(column);
        row[column] = inspect(cell, { depth: 1 });
      }
    } else if (Array.isArray(value)) {
      value.forEach((cell, i) => {
        valueColumns.add(String(i));
        row[String(i)] = inspect(cell, { depth: 1 });
      });
    } else {
      hasValues = true;
      row.Values = inspect(value, { depth: 1 });
    }
    rows.push(row);
  }

  let headers = ["(index)", ...valueColumns];
  if (hasValues) headers.push("Values");
  if (Array.isArray(columns) && columns.length > 0) {
    headers = ["(index)", ...columns.map(String)];
  }

  const widths = headers.map((header) =>
    Math.max(header.length, ...rows.map((row) => (row[header] ?? "").length), 0),
  );

  const line = (left, mid, right) =>
    left + widths.map((width) => "\u2500".repeat(width + 2)).join(mid) + right;
  const renderRow = (cells) =>
    "\u2502" + cells.map((cell, i) => ` ${String(cell).padEnd(widths[i])} `).join("\u2502") + "\u2502";

  const out = [line("\u250c", "\u252c", "\u2510"), renderRow(headers), line("\u251c", "\u253c", "\u2524")];
  for (const row of rows) {
    out.push(renderRow(headers.map((header) => row[header] ?? "")));
  }
  out.push(line("\u2514", "\u2534", "\u2518"));
  return out.join("\n");
}

const console = {
  log(...args) {
    write(argsToString(args), false);
  },
  info(...args) {
    write(argsToString(args), false);
  },
  debug(...args) {
    write(argsToString(args), false);
  },
  dir(value, options = {}) {
    write(inspect(value, { depth: options.depth ?? 4 }), false);
  },
  warn(...args) {
    write(argsToString(args), true);
  },
  error(...args) {
    write(argsToString(args), true);
  },

  trace(...args) {
    const message = argsToString(args);
    const error = new Error();
    // Drop the `Error` header and this frame itself.
    const stack = (error.stack ?? "").split("\n").slice(2).join("\n");
    write(`Trace${message ? `: ${message}` : ""}\n${stack}`, true);
  },

  assert(condition, ...args) {
    if (condition) return;
    const message = argsToString(args);
    write(`Assertion failed${message ? `: ${message}` : ""}`, true);
  },

  table(data, columns) {
    if (data === null || typeof data !== "object") {
      write(inspect(data), false);
      return;
    }
    write(buildTable(data, columns), false);
  },

  group(...args) {
    if (args.length > 0) write(argsToString(args), false);
    groupIndent += "  ";
  },
  groupCollapsed(...args) {
    console.group(...args);
  },
  groupEnd() {
    groupIndent = groupIndent.slice(0, -2);
  },

  count(label = "default") {
    const next = (counters.get(label) ?? 0) + 1;
    counters.set(label, next);
    write(`${label}: ${next}`, false);
  },
  countReset(label = "default") {
    counters.delete(label);
  },

  time(label = "default") {
    if (timers.has(label)) {
      write(`Warning: Label '${label}' already exists for console.time()`, true);
      return;
    }
    timers.set(label, performance.now());
  },
  timeLog(label = "default", ...args) {
    const start = timers.get(label);
    if (start === undefined) {
      write(`Warning: No such label '${label}' for console.timeLog()`, true);
      return;
    }
    const suffix = args.length > 0 ? ` ${argsToString(args)}` : "";
    write(`${label}: ${formatDuration(performance.now() - start)}${suffix}`, false);
  },
  timeEnd(label = "default") {
    const start = timers.get(label);
    if (start === undefined) {
      write(`Warning: No such label '${label}' for console.timeEnd()`, true);
      return;
    }
    timers.delete(label);
    write(`${label}: ${formatDuration(performance.now() - start)}`, false);
  },

  clear() {
    groupIndent = "";
    core.print("\x1b[1;1H\x1b[0J", false);
  },
};

function formatDuration(milliseconds) {
  if (milliseconds < 1000) return `${milliseconds.toFixed(3)}ms`;
  if (milliseconds < 60_000) return `${(milliseconds / 1000).toFixed(3)}s`;
  const minutes = Math.floor(milliseconds / 60_000);
  return `${minutes}m ${((milliseconds % 60_000) / 1000).toFixed(3)}s`;
}

export { console, inspect, setColorsEnabled };
