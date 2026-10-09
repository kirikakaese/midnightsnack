// SPDX-License-Identifier: GPL-3.0-or-later
// Noise_NK_25519_ChaChaPoly_BLAKE2s (https://noiseprotocol.org/noise.html), just what the relay
// tunnel needs: the remote is the initiator and knows the host's static key from the join link.
// Built on the audited @noble primitives; checked against `snow` with fixed vectors.
import { chacha20poly1305 } from "@noble/ciphers/chacha.js";
import { x25519 } from "@noble/curves/ed25519.js";
import { blake2s } from "@noble/hashes/blake2.js";
import { hmac } from "@noble/hashes/hmac.js";

const PROTOCOL_NAME = "Noise_NK_25519_ChaChaPoly_BLAKE2s";
const HASHLEN = 32;
const encoder = new TextEncoder();

export class NoiseError extends Error {}

function concat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let i = 0;
  for (const p of parts) {
    out.set(p, i);
    i += p.length;
  }
  return out;
}

function hash(data: Uint8Array): Uint8Array {
  return blake2s(data);
}

function hkdf2(ck: Uint8Array, ikm: Uint8Array): [Uint8Array, Uint8Array] {
  const temp = hmac(blake2s, ck, ikm);
  const out1 = hmac(blake2s, temp, Uint8Array.of(1));
  const out2 = hmac(blake2s, temp, concat(out1, Uint8Array.of(2)));
  return [out1, out2];
}

/** One direction of a Noise session. */
export class CipherState {
  #k: Uint8Array | null;
  #n = 0;

  constructor(k: Uint8Array | null = null) {
    this.#k = k;
  }

  get hasKey(): boolean {
    return this.#k !== null;
  }

  #nonce(): Uint8Array {
    if (this.#n >= Number.MAX_SAFE_INTEGER) throw new NoiseError("nonce exhausted");
    const nonce = new Uint8Array(12);
    // 4 zero bytes, then the 64-bit counter little-endian.
    new DataView(nonce.buffer).setBigUint64(4, BigInt(this.#n), true);
    return nonce;
  }

  encrypt(ad: Uint8Array, plaintext: Uint8Array): Uint8Array {
    if (!this.#k) return plaintext;
    const out = chacha20poly1305(this.#k, this.#nonce(), ad).encrypt(plaintext);
    this.#n++;
    return out;
  }

  decrypt(ad: Uint8Array, ciphertext: Uint8Array): Uint8Array {
    if (!this.#k) return ciphertext;
    let out: Uint8Array;
    try {
      out = chacha20poly1305(this.#k, this.#nonce(), ad).decrypt(ciphertext);
    } catch {
      throw new NoiseError("decryption failed");
    }
    this.#n++;
    return out;
  }
}

class SymmetricState {
  h: Uint8Array;
  ck: Uint8Array;
  cipher = new CipherState();

  constructor() {
    const name = encoder.encode(PROTOCOL_NAME);
    if (name.length <= HASHLEN) {
      this.h = new Uint8Array(HASHLEN);
      this.h.set(name);
    } else {
      this.h = hash(name);
    }
    this.ck = this.h;
  }

  mixHash(data: Uint8Array): void {
    this.h = hash(concat(this.h, data));
  }

  mixKey(ikm: Uint8Array): void {
    const [ck, k] = hkdf2(this.ck, ikm);
    this.ck = ck;
    this.cipher = new CipherState(k);
  }

  encryptAndHash(plaintext: Uint8Array): Uint8Array {
    const c = this.cipher.encrypt(this.h, plaintext);
    this.mixHash(c);
    return c;
  }

  decryptAndHash(ciphertext: Uint8Array): Uint8Array {
    const p = this.cipher.decrypt(this.h, ciphertext);
    this.mixHash(ciphertext);
    return p;
  }

  split(): [CipherState, CipherState] {
    const [k1, k2] = hkdf2(this.ck, new Uint8Array(0));
    return [new CipherState(k1), new CipherState(k2)];
  }
}

/** Encryption both ways after the handshake. */
export interface NoiseTransport {
  send: CipherState;
  receive: CipherState;
}

function initialize(prologue: Uint8Array, responderStatic: Uint8Array): SymmetricState {
  const ss = new SymmetricState();
  ss.mixHash(prologue);
  // Pre-message pattern `<- s`.
  ss.mixHash(responderStatic);
  return ss;
}

/** The remote's side: `-> e, es` then `<- e, ee`. */
export class NkInitiator {
  #ss: SymmetricState;
  #rs: Uint8Array;
  #e: Uint8Array;

  /** `ephemeral` is for test vectors only; normally a fresh key is generated. */
  constructor(prologue: Uint8Array, responderStatic: Uint8Array, ephemeral?: Uint8Array) {
    if (responderStatic.length !== 32) throw new NoiseError("bad host key");
    this.#rs = responderStatic;
    this.#ss = initialize(prologue, responderStatic);
    this.#e = ephemeral ?? x25519.utils.randomSecretKey();
  }

  writeMessage1(payload: Uint8Array = new Uint8Array(0)): Uint8Array {
    const ePub = x25519.getPublicKey(this.#e);
    this.#ss.mixHash(ePub);
    this.#ss.mixKey(x25519.getSharedSecret(this.#e, this.#rs));
    return concat(ePub, this.#ss.encryptAndHash(payload));
  }

  /** Reads the host's answer; throws {@link NoiseError} if it is not from the expected host. */
  readMessage2(message: Uint8Array): { payload: Uint8Array; transport: NoiseTransport } {
    if (message.length < 32 + 16) throw new NoiseError("short handshake message");
    const re = message.subarray(0, 32);
    this.#ss.mixHash(re);
    this.#ss.mixKey(x25519.getSharedSecret(this.#e, re));
    const payload = this.#ss.decryptAndHash(message.subarray(32));
    const [send, receive] = this.#ss.split();
    return { payload, transport: { send, receive } };
  }
}

/** The host's side, used by tests (the real host uses `snow`). */
export class NkResponder {
  #ss: SymmetricState;
  #s: Uint8Array;
  #e: Uint8Array;
  #re: Uint8Array | null = null;

  constructor(prologue: Uint8Array, staticPrivate: Uint8Array, ephemeral?: Uint8Array) {
    this.#s = staticPrivate;
    this.#ss = initialize(prologue, x25519.getPublicKey(staticPrivate));
    this.#e = ephemeral ?? x25519.utils.randomSecretKey();
  }

  readMessage1(message: Uint8Array): Uint8Array {
    if (message.length < 32 + 16) throw new NoiseError("short handshake message");
    const re = message.subarray(0, 32);
    this.#re = re;
    this.#ss.mixHash(re);
    this.#ss.mixKey(x25519.getSharedSecret(this.#s, re));
    return this.#ss.decryptAndHash(message.subarray(32));
  }

  writeMessage2(payload: Uint8Array = new Uint8Array(0)): {
    message: Uint8Array;
    transport: NoiseTransport;
  } {
    if (!this.#re) throw new NoiseError("message 1 not read");
    const ePub = x25519.getPublicKey(this.#e);
    this.#ss.mixHash(ePub);
    this.#ss.mixKey(x25519.getSharedSecret(this.#e, this.#re));
    const message = concat(ePub, this.#ss.encryptAndHash(payload));
    const [c1, c2] = this.#ss.split();
    return { message, transport: { send: c2, receive: c1 } };
  }
}

/** Transport messages carry no associated data. */
export const NO_AD = new Uint8Array(0);

export function base64urlDecode(s: string): Uint8Array {
  const b64 = s.replace(/-/g, "+").replace(/_/g, "/");
  const padded = b64 + "=".repeat((4 - (b64.length % 4)) % 4);
  const bin = atob(padded);
  return Uint8Array.from(bin, (c) => c.charCodeAt(0));
}
