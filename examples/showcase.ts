/// <reference types="../types/chaqmoq.d.ts" />
//
// A tour of the console inspector: how Chaqmoq renders every kind of value.
//
//   chaqmoq run examples/showcase.ts

console.log("--- primitives ---");
console.log(42, -0, 3.14, 10n, true, null, undefined, Symbol("id"));

console.log("\n--- collections ---");
console.log({
  name: "chaqmoq",
  tags: ["runtime", "v8", "rust"],
  meta: new Map<string, unknown>([
    ["stars", 0],
    ["license", "MIT"],
  ]),
  unique: new Set([1, 2, 2, 3]),
  bytes: new Uint8Array([0xde, 0xad, 0xbe, 0xef]),
  when: new Date("2026-01-01T00:00:00Z"),
  pattern: /chaqmoq/gi,
});

console.log("\n--- class instances ---");
class Point {
  constructor(public x: number, public y: number) {}
  get length() {
    return Math.hypot(this.x, this.y);
  }
}
console.log(new Point(3, 4));

console.log("\n--- circular references ---");
const node: Record<string, unknown> = { id: 1 };
node.self = node;
console.log(node);

console.log("\n--- format specifiers ---");
console.log("%s scored %d points (%f%% accuracy)", "chaqmoq", 42, 99.5);

console.log("\n--- tables ---");
console.table([
  { command: "run", description: "run a program", stability: "stable" },
  { command: "test", description: "run tests", stability: "stable" },
  { command: "repl", description: "interactive shell", stability: "stable" },
]);

console.log("\n--- grouping and timing ---");
console.group("startup");
console.log("loading modules");
console.group("permissions");
console.log("read: denied");
console.log("net: denied");
console.groupEnd();
console.groupEnd();

console.time("work");
let total = 0;
for (let i = 0; i < 1_000_000; i++) total += i;
console.timeEnd("work");
console.log("total:", total);
