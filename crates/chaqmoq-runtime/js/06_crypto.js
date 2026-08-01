// `crypto` -- the subset of Web Crypto that does not require key material.

import {
  op_chq_digest,
  op_chq_get_random_values,
  op_chq_random_uuid,
} from "ext:core/ops";

const INTEGER_VIEWS = [
  Int8Array,
  Uint8Array,
  Uint8ClampedArray,
  Int16Array,
  Uint16Array,
  Int32Array,
  Uint32Array,
  BigInt64Array,
  BigUint64Array,
];

function getRandomValues(view) {
  if (!ArrayBuffer.isView(view) || !INTEGER_VIEWS.some((kind) => view instanceof kind)) {
    throw new TypeError(
      "Argument must be an integer-typed TypedArray (Float views are not allowed)",
    );
  }
  // Fill the underlying bytes; the caller's view reinterprets them.
  const bytes = new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
  op_chq_get_random_values(bytes);
  return view;
}

const DIGEST_ALIASES = {
  "sha-1": "SHA-1",
  sha1: "SHA-1",
  "sha-256": "SHA-256",
  sha256: "SHA-256",
  "sha-384": "SHA-384",
  sha384: "SHA-384",
  "sha-512": "SHA-512",
  sha512: "SHA-512",
};

function normalizeAlgorithm(algorithm) {
  const name = typeof algorithm === "string" ? algorithm : algorithm?.name;
  const normalized = DIGEST_ALIASES[String(name).toLowerCase()];
  if (!normalized) {
    throw new TypeError(`Unrecognized algorithm name: ${name}`);
  }
  return normalized;
}

function toBytes(data) {
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (ArrayBuffer.isView(data)) {
    return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  }
  throw new TypeError("data must be an ArrayBuffer or ArrayBufferView");
}

const subtle = {
  // Async to match the Web Crypto signature, even though the digest itself is
  // computed synchronously -- the payloads that reach here are small.
  async digest(algorithm, data) {
    const result = op_chq_digest(normalizeAlgorithm(algorithm), toBytes(data));
    return result.buffer;
  },
};

const crypto = {
  getRandomValues,
  randomUUID() {
    return op_chq_random_uuid();
  },
  subtle,
  get [Symbol.toStringTag]() {
    return "Crypto";
  },
};

export { crypto };
