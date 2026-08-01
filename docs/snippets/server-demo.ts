/// <reference types="../../types/chaqmoq.d.ts" />
//
// A self-contained server round trip: start, serve three requests from the same
// process, then shut down. Used by docs/tools/capture.sh so the screenshot shows
// both sides of the exchange without a second terminal.

const server = Chaqmoq.serve({
  port: 0,
  hostname: "127.0.0.1",
  onListen: ({ port }) => console.log(`server listening on http://127.0.0.1:${port}/`),
}, (request) => {
  const { pathname } = new URL(request.url);
  console.log(`  <- ${request.method} ${pathname}`);

  if (pathname === "/health") {
    return Response.json({ status: "ok", uptime: Number(Chaqmoq.osUptime().toFixed(3)) });
  }
  if (pathname === "/echo" && request.method === "POST") {
    return request.text().then((body) => new Response(body.toUpperCase(), { status: 201 }));
  }
  return new Response("Salom from Chaqmoq!");
});

const base = `http://127.0.0.1:${server.addr.port}`;

const root = await fetch(base);
console.log(`  -> ${root.status} ${JSON.stringify(await root.text())}`);

const health = await fetch(`${base}/health`);
console.log(`  -> ${health.status}`, await health.json());

const echo = await fetch(`${base}/echo`, { method: "POST", body: "salom dunyo" });
console.log(`  -> ${echo.status} ${JSON.stringify(await echo.text())}`);

server.shutdown();
console.log("server stopped");
