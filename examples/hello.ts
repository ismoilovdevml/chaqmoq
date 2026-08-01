/// <reference types="../types/chaqmoq.d.ts" />
//
// The smallest useful program: types, template literals, and the runtime's
// value inspector.
//
//   chaqmoq run examples/hello.ts

interface Greeting {
  language: string;
  text: string;
}

const greetings: Greeting[] = [
  { language: "uz", text: "Salom" },
  { language: "en", text: "Hello" },
  { language: "ru", text: "Privet" },
];

for (const { language, text } of greetings) {
  console.log(`[${language}] ${text}, Chaqmoq!`);
}

console.log("\nRuntime:", Chaqmoq.version);
console.log("Platform:", `${Chaqmoq.build.arch}-${Chaqmoq.build.os}`);
