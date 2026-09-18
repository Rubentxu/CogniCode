# e88 — Verification Report

## Entry gates (public v0.97.0 bits)
- E88-G0a PASS: public cogh resolves public release; no DEV fixture; tracker correct.
- E88-G0b PASS: published binary doctor healthy via canonical shim.

## Findings -> fixes -> releases
- F1 rollback stale tracker pin: journal persisted before WroteTracker
  recorded. Fixed c795e964; published in v0.97.1.
- F2 doctor UNHEALTHY after clean uninstall: MCP probe turned absent shim
  into Fail regardless of installed state. Fixed 89b19019 with the
  state-aware matrix (no pin -> Unavailable; declared+shim -> Pass;
  declared+missing -> Fail; undeclared -> Unavailable). Published v0.97.1.
- F3 (new, non-blocking): idempotent same-version update overwrites the
  journal; rollback of that transition leaves pin-without-tree. Recovery =
  reinstall. Recorded for a future lifecycle cycle (DEBT-4 one-shot journal).

## Final acceptance (public v0.97.1, disposable HOME, zero seams)
docs/e88-final-uat-v0.97.1.md — U0..U8 all GREEN:
bootstrap, install, doctor, update, rollback (pin restored), reinstall,
uninstall, idempotence, mise declarative backend. Real HOME untouched.

## Result
e88 CLOSED. Distribution objective met: a user without a checkout can
install, diagnose, update, revert and uninstall using only published
surfaces.
