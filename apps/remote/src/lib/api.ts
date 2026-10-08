// SPDX-License-Identifier: GPL-3.0-or-later
import type { ApiError, ErrorCode, PairResponse, PairStatus } from "@midnightsnack/protocol";

async function errorCode(res: Response): Promise<ErrorCode> {
  try {
    return ((await res.json()) as ApiError).code;
  } catch {
    return "internal";
  }
}

export async function requestPairing(
  join_token: string,
  pin: string,
  device_name: string,
): Promise<{ ok: true; requestId: string } | { ok: false; code: ErrorCode }> {
  try {
    const res = await fetch("/api/v1/pair", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ join_token, pin, device_name }),
    });
    if (!res.ok) return { ok: false, code: await errorCode(res) };
    return { ok: true, requestId: ((await res.json()) as PairResponse).request_id };
  } catch {
    return { ok: false, code: "internal" };
  }
}

export async function pairingStatus(requestId: string): Promise<PairStatus | null> {
  try {
    const res = await fetch(`/api/v1/pair/${encodeURIComponent(requestId)}`);
    if (!res.ok) return null;
    return (await res.json()) as PairStatus;
  } catch {
    return null;
  }
}
