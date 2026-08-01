/// <reference types="../types/chaqmoq.d.ts" />
//
// Filesystem APIs, and what the permission errors look like when you forget a
// flag.
//
//   chaqmoq run --allow-read --allow-write examples/filesystem.ts

const dir = await Chaqmoq.makeTempDir({ prefix: "chaqmoq_demo_" });
console.log("working in", dir);

await Chaqmoq.writeTextFile(`${dir}/notes.md`, "# Chaqmoq\n");
await Chaqmoq.writeTextFile(`${dir}/notes.md`, "\nBuilt on V8 and Rust.\n", {
  append: true,
});
await Chaqmoq.mkdir(`${dir}/src/deep`, { recursive: true });
await Chaqmoq.writeTextFile(`${dir}/src/deep/index.ts`, "export const ok = true;\n");

console.log("\ncontents of notes.md:");
console.log(await Chaqmoq.readTextFile(`${dir}/notes.md`));

const info = await Chaqmoq.stat(`${dir}/notes.md`);
console.log("notes.md:", {
  size: info.size,
  isFile: info.isFile,
  modified: info.mtime ? new Date(info.mtime).toISOString() : null,
});

console.log("\ntree:");
for (const entry of await Chaqmoq.readDir(dir)) {
  console.log(`  ${entry.isDirectory ? "d" : "-"} ${entry.name}`);
}

// A permission failure is a normal, catchable error.
try {
  await Chaqmoq.readTextFile("/etc/shadow");
} catch (error) {
  console.log("\nreading a file outside the grant:");
  console.log(" ", (error as Error).message);
}

await Chaqmoq.remove(dir, { recursive: true });
console.log("\ncleaned up:", !(await Chaqmoq.exists(dir)));
