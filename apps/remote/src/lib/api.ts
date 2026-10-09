// SPDX-License-Identifier: GPL-3.0-or-later
import type { ApiError, ErrorCode, PairResponse, PairStatus } from "@midnightsnack/protocol";
import { jsonBody, type HostTransport, type TransportFailure } from "@midnightsnack/ui";

export type PairingFailure = ErrorCode | TransportFailure;

export async function requestPairing(
  transport: HostTransport,
  join_token: string,
  pin: string,
  device_name: string,
): Promise<{ ok: true; requestId: string } | { ok: false; code: PairingFailure }> {
  try {
    const res = await transport.request("POST", "/api/v1/pair", {
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ join_token, pin, device_name }),
    });
    if (res.status !== 200) {
      return { ok: false, code: jsonBody<ApiError>(res)?.code ?? "internal" };
    }
    const body = jsonBody<PairResponse>(res);
    return body ? { ok: true, requestId: body.request_id } : { ok: false, code: "internal" };
  } catch (e) {
    // Relay failures arrive as their reason.
    return { ok: false, code: typeof e === "string" ? (e as TransportFailure) : "internal" };
  }
}

export async function pairingStatus(
  transport: HostTransport,
  requestId: string,
): Promise<PairStatus | null | "retry"> {
  try {
    const res = await transport.request("GET", `/api/v1/pair/${encodeURIComponent(requestId)}`);
    if (res.status !== 200) return null;
    return jsonBody<PairStatus>(res);
  } catch {
    // The connection dropped; keep waiting.
    return "retry";
  }
}
