/// <reference types="../../types/chaqmoq.d.ts" />
//
// Used by docs/tools/capture.sh to show the permission model: the same program,
// run once without a grant and once with a scoped one.

const path = "/etc/hosts";
const text = await Chaqmoq.readTextFile(path);
console.log(`read ${text.length} bytes from ${path}`);
