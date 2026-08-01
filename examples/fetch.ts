/// <reference types="../types/chaqmoq.d.ts" />
//
// The HTTP client. Note that `--allow-net` can be scoped to specific hosts, so
// a program that only talks to one API cannot quietly call another.
//
//   chaqmoq run --allow-net=api.github.com examples/fetch.ts denoland/deno

const repo = Chaqmoq.args[0] ?? "denoland/deno";

const response = await fetch(`https://api.github.com/repos/${repo}`, {
  headers: { accept: "application/vnd.github+json" },
});

if (!response.ok) {
  console.error(`request failed: ${response.status} ${response.statusText}`);
  Chaqmoq.exit(1);
}

const data = await response.json() as {
  full_name: string;
  description: string | null;
  stargazers_count: number;
  language: string | null;
};

console.log({
  repository: data.full_name,
  description: data.description,
  stars: data.stargazers_count,
  language: data.language,
});

console.log("\nresponse headers:");
for (const [name, value] of response.headers) {
  if (name.startsWith("x-ratelimit")) console.log(`  ${name}: ${value}`);
}
