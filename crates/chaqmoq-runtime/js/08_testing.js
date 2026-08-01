// The built-in test runner behind `chaqmoq test`.
//
// Tests register themselves at module evaluation time and the CLI runs them
// afterwards, which is what lets a plain `chaqmoq run` on a test file simply do
// nothing instead of failing.

import { core } from "ext:core/mod.js";
import { performance } from "ext:chaqmoq/01_web.js";

/** @type {Array<{name: string, fn: Function, ignore: boolean, only: boolean}>} */
const registered = [];

/**
 * Registers a test.
 *
 * ```js
 * Chaqmoq.test("adds", () => { if (1 + 1 !== 2) throw new Error("math"); });
 * Chaqmoq.test({ name: "slow", ignore: true }, async () => {});
 * ```
 */
function test(definitionOrName, maybeFn) {
  let definition;

  if (typeof definitionOrName === "string") {
    definition = { name: definitionOrName, fn: maybeFn };
  } else if (typeof definitionOrName === "function") {
    definition = { name: definitionOrName.name, fn: definitionOrName };
  } else if (definitionOrName && typeof definitionOrName === "object") {
    definition = { ...definitionOrName, fn: maybeFn ?? definitionOrName.fn };
  } else {
    throw new TypeError("Chaqmoq.test expects a name and a function, or a definition object");
  }

  if (typeof definition.fn !== "function") {
    throw new TypeError(`Test "${definition.name}" has no function body`);
  }
  if (!definition.name) {
    throw new TypeError("Tests must be named");
  }

  registered.push({
    name: String(definition.name),
    fn: definition.fn,
    ignore: Boolean(definition.ignore),
    only: Boolean(definition.only),
  });
}

const GREEN = "\x1b[32m";
const RED = "\x1b[31m";
const YELLOW = "\x1b[33m";
const DIM = "\x1b[2m";
const RESET = "\x1b[0m";

/**
 * Runs everything registered so far and returns a summary.
 *
 * Called by the CLI once all test modules have been evaluated.
 *
 * @param {{color?: boolean, filter?: string}} options
 */
async function runRegisteredTests(options = {}) {
  const color = options.color ?? false;
  const paint = (code, text) => (color ? `${code}${text}${RESET}` : text);

  // `only` is a focus switch: if any test uses it, everything else is skipped.
  const focused = registered.filter((t) => t.only);
  let selected = focused.length > 0 ? focused : registered;

  if (options.filter) {
    selected = selected.filter((t) => t.name.includes(options.filter));
  }

  const results = { passed: 0, failed: 0, ignored: 0, failures: [] };
  const startedAt = performance.now();

  for (const entry of selected) {
    if (entry.ignore) {
      results.ignored++;
      core.print(`${entry.name} ... ${paint(YELLOW, "ignored")}\n`, false);
      continue;
    }

    const testStart = performance.now();
    try {
      await entry.fn();
      const elapsed = (performance.now() - testStart).toFixed(0);
      results.passed++;
      core.print(
        `${entry.name} ... ${paint(GREEN, "ok")} ${paint(DIM, `(${elapsed}ms)`)}\n`,
        false,
      );
    } catch (error) {
      const elapsed = (performance.now() - testStart).toFixed(0);
      results.failed++;
      results.failures.push({ name: entry.name, error });
      core.print(
        `${entry.name} ... ${paint(RED, "FAILED")} ${paint(DIM, `(${elapsed}ms)`)}\n`,
        false,
      );
    }
  }

  if (results.failures.length > 0) {
    core.print(`\n${paint(RED, "failures:")}\n\n`, false);
    for (const failure of results.failures) {
      core.print(`${failure.name}\n`, false);
      const stack = failure.error?.stack ?? String(failure.error);
      core.print(`${stack}\n\n`, false);
    }
  }

  const elapsed = ((performance.now() - startedAt) / 1000).toFixed(2);
  const verdict = results.failed === 0 ? paint(GREEN, "ok") : paint(RED, "FAILED");
  core.print(
    `\ntest result: ${verdict}. ${results.passed} passed; ${results.failed} failed; ` +
      `${results.ignored} ignored ${paint(DIM, `(${elapsed}s)`)}\n`,
    false,
  );

  return results;
}

function registeredCount() {
  return registered.length;
}

export { registeredCount, runRegisteredTests, test };
