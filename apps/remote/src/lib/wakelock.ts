// SPDX-License-Identifier: GPL-3.0-or-later
// Keeps the phone screen on while the remote is visible (where supported).
export function keepScreenOn(): () => void {
  let lock: WakeLockSentinel | null = null;
  const request = async () => {
    try {
      if (document.visibilityState === "visible" && "wakeLock" in navigator) {
        lock = await navigator.wakeLock.request("screen");
      }
    } catch {
      // Not allowed (e.g. low battery); the remote still works.
    }
  };
  const onVisible = () => void request();
  document.addEventListener("visibilitychange", onVisible);
  void request();
  return () => {
    document.removeEventListener("visibilitychange", onVisible);
    void lock?.release();
  };
}

export function tap(): void {
  try {
    navigator.vibrate?.(8);
  } catch {
    // Vibration unsupported.
  }
}
