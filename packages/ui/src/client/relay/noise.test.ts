// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { NkInitiator, NkResponder, NO_AD, NoiseError, base64urlDecode } from "./noise";
import vectors from "./noise-vectors.json";

const hex = (s: string) => Uint8Array.from(s.match(/../g) ?? [], (b) => parseInt(b, 16));
const toHex = (b: Uint8Array) => Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
const prologue = new TextEncoder().encode(vectors.prologue);

describe("Noise NK", () => {
  it("matches the vectors produced by snow (the host's implementation)", () => {
    const i = new NkInitiator(
      prologue,
      hex(vectors.responder_static_public),
      hex(vectors.initiator_ephemeral),
    );
    const r = new NkResponder(
      prologue,
      hex(vectors.responder_static_private),
      hex(vectors.responder_ephemeral),
    );
    const m1 = i.writeMessage1();
    expect(toHex(m1)).toBe(vectors.message1);
    r.readMessage1(m1);
    const { message: m2, transport: rt } = r.writeMessage2();
    expect(toHex(m2)).toBe(vectors.message2);
    const { transport: it_ } = i.readMessage2(m2);
    for (const [who, plain, cipher] of vectors.transport as [string, string, string][]) {
      const [tx, rx] = who === "initiator" ? [it_, rt] : [rt, it_];
      const c = tx.send.encrypt(NO_AD, hex(plain));
      expect(toHex(c)).toBe(cipher);
      expect(toHex(rx.receive.decrypt(NO_AD, c))).toBe(plain);
    }
  });

  it("refuses a host with another key and tampered messages", () => {
    const real = new NkResponder(prologue, hex(vectors.responder_static_private));
    const wrongKey = new Uint8Array(32).fill(9);
    const i = new NkInitiator(prologue, wrongKey);
    expect(() => real.readMessage1(i.writeMessage1())).toThrow(NoiseError);

    const i2 = new NkInitiator(prologue, hex(vectors.responder_static_public));
    const r2 = new NkResponder(prologue, hex(vectors.responder_static_private));
    r2.readMessage1(i2.writeMessage1());
    const { message } = r2.writeMessage2();
    message[40] = message[40]! ^ 1;
    expect(() => i2.readMessage2(message)).toThrow(NoiseError);
  });

  it("detects replayed or reordered transport messages", () => {
    const i = new NkInitiator(prologue, hex(vectors.responder_static_public));
    const r = new NkResponder(prologue, hex(vectors.responder_static_private));
    r.readMessage1(i.writeMessage1());
    const { message, transport: rt } = r.writeMessage2();
    const { transport: it_ } = i.readMessage2(message);
    const a = it_.send.encrypt(NO_AD, Uint8Array.of(1));
    const b = it_.send.encrypt(NO_AD, Uint8Array.of(2));
    expect(() => rt.receive.decrypt(NO_AD, b)).toThrow(NoiseError);
    void a;
  });

  it("decodes base64url keys", () => {
    expect(Array.from(base64urlDecode("_-8"))).toEqual([0xff, 0xef]);
  });
});
