/// <reference types="../types/chaqmoq.d.ts" />
//
// A small JSON API with routing, request bodies and error handling.
//
//   chaqmoq run --allow-net=0.0.0.0:8000 examples/http-server.ts
//
// Then:
//   curl localhost:8000/
//   curl localhost:8000/api/tasks
//   curl -X POST localhost:8000/api/tasks -d '{"title":"write docs"}'

interface Task {
  id: number;
  title: string;
  done: boolean;
}

const tasks: Task[] = [
  { id: 1, title: "build a runtime", done: true },
  { id: 2, title: "write the README", done: false },
];
let nextId = tasks.length + 1;

const routes: Record<string, (request: Request) => Response | Promise<Response>> = {
  "GET /": () =>
    new Response(
      [
        "Chaqmoq HTTP server",
        "",
        "  GET  /api/tasks       list tasks",
        "  POST /api/tasks       create a task  {\"title\": \"...\"}",
        "  GET  /api/health      liveness probe",
        "",
      ].join("\n"),
      { headers: { "content-type": "text/plain; charset=utf-8" } },
    ),

  "GET /api/health": () =>
    Response.json({
      status: "ok",
      uptime: Number(Chaqmoq.osUptime().toFixed(3)),
      runtime: Chaqmoq.version,
    }),

  "GET /api/tasks": () => Response.json({ tasks, count: tasks.length }),

  "POST /api/tasks": async (request) => {
    const payload = (await request.json()) as Partial<Task>;
    if (typeof payload.title !== "string" || payload.title.trim() === "") {
      return Response.json({ error: "title is required" }, { status: 400 });
    }
    const task: Task = { id: nextId++, title: payload.title.trim(), done: false };
    tasks.push(task);
    return Response.json(task, { status: 201 });
  },
};

const server = Chaqmoq.serve({
  port: 8000,
  onListen: ({ hostname, port }) => {
    const host = hostname === "0.0.0.0" ? "localhost" : hostname;
    console.log(`Chaqmoq serving on http://${host}:${port}/`);
  },
  onError: (error) => {
    console.error("handler failed:", error);
    return Response.json({ error: "internal server error" }, { status: 500 });
  },
}, async (request) => {
  const { pathname } = new URL(request.url);
  const route = routes[`${request.method} ${pathname}`];

  const started = performance.now();
  const response = route
    ? await route(request)
    : Response.json({ error: "not found", path: pathname }, { status: 404 });

  const elapsed = (performance.now() - started).toFixed(1);
  console.log(`${request.method} ${pathname} -> ${response.status} (${elapsed}ms)`);
  return response;
});

await server.finished;
