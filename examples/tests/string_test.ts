/// <reference types="../../types/chaqmoq.d.ts" />
//
// Tests are plain modules that register cases with `Chaqmoq.test`.
//
//   chaqmoq test examples/tests/

function assertEquals<T>(actual: T, expected: T, message?: string): void {
  if (!Object.is(actual, expected)) {
    throw new Error(
      message ?? `expected ${Chaqmoq.inspect(expected)}, got ${Chaqmoq.inspect(actual)}`,
    );
  }
}

function assertThrows(fn: () => unknown, message?: string): void {
  try {
    fn();
  } catch {
    return;
  }
  throw new Error(message ?? "expected the function to throw");
}

Chaqmoq.test("URL parses query parameters", () => {
  const url = new URL("https://chaqmoq.dev/search?q=runtime&lang=uz");
  assertEquals(url.hostname, "chaqmoq.dev");
  assertEquals(url.searchParams.get("q"), "runtime");
  assertEquals(url.searchParams.get("lang"), "uz");
});

Chaqmoq.test("URLSearchParams writes back to its URL", () => {
  const url = new URL("https://chaqmoq.dev/");
  url.searchParams.set("page", "2");
  assertEquals(url.href, "https://chaqmoq.dev/?page=2");
});

Chaqmoq.test("base64 round-trips", () => {
  const original = "Chaqmoq runtime";
  assertEquals(atob(btoa(original)), original);
});

Chaqmoq.test("TextEncoder and TextDecoder round-trip UTF-8", () => {
  const bytes = new TextEncoder().encode("salom, dunyo");
  assertEquals(new TextDecoder().decode(bytes), "salom, dunyo");
});

Chaqmoq.test("crypto.randomUUID produces distinct v4 UUIDs", () => {
  const a = crypto.randomUUID();
  const b = crypto.randomUUID();
  assertEquals(a.length, 36);
  assertEquals(a[14], "4");
  assertEquals(a === b, false);
});

Chaqmoq.test("timers resolve in order", async () => {
  const order: number[] = [];
  await Promise.all([
    new Promise<void>((resolve) => setTimeout(() => (order.push(2), resolve()), 20)),
    new Promise<void>((resolve) => setTimeout(() => (order.push(1), resolve()), 5)),
  ]);
  assertEquals(order.join(","), "1,2");
});

Chaqmoq.test("Response.json sets a JSON content type", async () => {
  const response = Response.json({ ok: true });
  assertEquals(response.headers.get("content-type"), "application/json; charset=utf-8");
  assertEquals(JSON.stringify(await response.json()), '{"ok":true}');
});

Chaqmoq.test("a body can only be consumed once", async () => {
  const response = new Response("payload");
  assertEquals(await response.text(), "payload");
  let threw = false;
  try {
    await response.text();
  } catch {
    threw = true;
  }
  assertEquals(threw, true);
});

Chaqmoq.test("reading a file without permission throws", async () => {
  // The test runner grants nothing unless you pass --allow-* flags.
  assertThrows(() => Chaqmoq.readTextFileSync("/etc/hosts"));
});
