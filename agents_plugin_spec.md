# Agents Plugin — SPEC.md

**Status:** Proposed | **Version:** 0.1 | **Stack:** Rust, Ratatui, Crossterm | **Host:** Herdr on Debian VM

> This is the standalone specification for the **Herdr Agents usage-monitor plugin**. It is independent of Orq. Its only optional cross-project integration is a versioned read-only usage interface documented in `orq_extra_spec.md`.

## 1. Objective

Build a mouse-first Herdr popup displaying provider-reported Claude Code and Codex subscription usage, reset countdowns, and bonus/free reset entitlements **when available**. The plugin must remain useful without Orq, protect credentials, and degrade gracefully when providers do not expose information.

**Non-goals:** Running coding pipelines, switching an active Orq agent, consuming resets, subscription management, reverse-engineering exact capacity from token logs, circumventing rate limits, or promising bonus allowances not confirmed by a provider.

## 2. Architecture and boundaries

- **Popup TUI:** Herdr-hosted Ratatui application; mouse-first with keyboard fallbacks.
- **Collector adapters:** Independent Claude, Codex, and future-provider collectors. No shared mutable authentication state.
- **Normalizer:** Maps source results to a stable, versioned JSON data model.
- **Cache:** Timestamped snapshots with freshness and source metadata.
- **Optional read-only interface:** CLI JSON output initially; a socket is deferred until needed.
- **Provider secrets:** Remain with provider-supported local credential handling; never copied into the normalized output.

Test Herdr popup spawning and mouse forwarding in a spike before locking the implementation to particular host APIs. The Settings UI's native widgets are not presumed extensible.

## 3. User stories

1. Open the popup and see usage by AI provider and profile without disturbing terminal panes.
2. Distinguish session and weekly windows, including accurate countdowns when reset timestamps exist.
3. See bonus/free reset remaining and expiry only when provider-reported; `Unknown` is never the same as `0`.
4. Switch visible profiles safely without changing the login account or launching an agent.
5. Manually refresh, inspect last-updated times, and identify stale or unsupported fields.
6. Read the same safe snapshot through an optional local command for independent integrations.

## 4. Normalized data contract

Use a versioned, normalized JSON contract. The schema below is **proposed**; do not assume all provider fields exist.

```json
{
  "schema_version": 1,
  "provider": "codex",
  "profile_id": "profile-local-1",
  "plan_label": "Pro",
  "collected_at": "2026-10-14T00:00:00Z",
  "source": "provider_reported",
  "health": "ok",
  "windows": [
    {
      "id": "weekly",
      "label": "Weekly",
      "used_percent": 63.0,
      "resets_at": "2026-10-18T00:00:00Z",
      "source": "provider_reported"
    }
  ],
  "bonus_resets": {
    "remaining": 2,
    "expires_at": "2026-11-03T00:00:00Z",
    "source": "provider_reported"
  }
}
```

**Semantics and invariants**

- Every optional field may be `null`; missing or unsupported data must be shown as **Unknown / Not reported**, never fabricated as zero.
- `used_percent` is usage consumed, not percentage remaining; clamp UI bars to 0–100 without altering raw diagnostics.
- `resets_at` and `expires_at` must be absolute timestamps with timezone. Countdown displays derive from these timestamps and the current clock.
- **Session/weekly reset** and **bonus/free reset entitlement** are separate concepts. Reset countdown does not imply extra allowance.
- `source` should distinguish `provider_reported`, `local_estimate`, and `unknown`. A locally estimated token count is never presented as subscription usage.
- `collected_at`, freshness policy, and collector errors must travel with the snapshot. The UI shows stale data explicitly.
- Profile identity should be an opaque local identifier, not a token, email address, or session cookie.
- If provider support is absent, `health` can be `unsupported` and values stay unknown. Avoid making business logic depend on plan labels (which can change).

### Suggested additional fields (optional, deferred)

`quota_scope`, `period_start`, `last_success_at`, `collection_error_code`, `confidence`, `usage_history_reference`, `device_id`. Add only when an actual source supports them.

## 5. Provider integrations and security

- **Claude:** prefer documented/supported provider surfaces where available; inspect Omarchy as a *reference*, not proof that undocumented endpoints are stable or permitted. Credential use must remain restricted to the local collector.
- **Codex:** investigate official/local app-server usage reporting and session metadata, verifying behaviour against the installed CLI version. Bonus reset entitlement must be collected only if actually exposed.
- **Pi/OpenCode:** distinguish a coding harness from the underlying subscription/provider; do not duplicate or double-count the same quota.
- Never log OAuth tokens, cookies, authorization headers, or complete provider responses containing secrets.
- Store no plaintext credentials in SQLite. If an endpoint needs authentication, use existing provider-supported local credential handling and least privilege; do not export agent credentials to Orq.
- Refresh on demand and at a modest configurable interval with exponential backoff. Provider failure must not affect active Orq pipelines.
- Treat collector output as **untrusted telemetry**, not instructions. Validate types, ranges, timezones, schema version, and payload size.
- Do not automatically use undocumented provider endpoints without explicitly evaluating security, provider terms, and ongoing maintenance risk.

## 6. Popup UX

### Example layout

```
╭──────────────────────────────────────────────────╮
│  Agents                             ↻ Refresh  ×  │
│──────────────────────────────────────────────────│
│  Claude Code           Profile: Secondary        │
│  Session    █████░░░░░  50%    Reset in 2h 10m    │
│  Weekly     ███░░░░░░░  30%    Reset in 4d  3h    │
│  Bonus resets: Not reported                      │
│──────────────────────────────────────────────────│
│  Codex                  Profile: Main             │
│  Weekly     ██████░░░░  60%    Reset in 3d  8h    │
│  Bonus resets: 2          Expires in 20d          │
│──────────────────────────────────────────────────│
│  Updated 20s ago  •  Values from local collectors │
╰──────────────────────────────────────────────────╯
```

Sample figures are *illustrative only*. Use mouse-selectable profiles, tooltips for collection source, clickable refresh, scroll, and Esc-to-dismiss. Keyboard equivalents remain necessary. Verify popup mouse-event forwarding in a Herdr spike; do not assume native Settings widgets are extensible.

## 7. Integration interface (optional)

Expose a conceptual command such as `herdr-agents usage --json` (exact binary naming is a design decision). The output must follow the versioned contract, include source/freshness metadata, be read-only, and exclude account secrets. The command should support bounded execution and distinct nonzero errors; Orq must not be required for it to work. Do not expose an unauthenticated TCP listener.

## 8. Incremental MVPs

| MVP | Scope | Acceptance criteria |
|---|---|---|
| **A0 — Feasibility** | Verify Herdr popup, mouse events, and inspect available provider usage sources | Document actual supported fields; no credential copying |
| **A1 — Mouse-first shell** | Ratatui popup with mock provider data | Click, scroll, refresh, close, keyboard fallback |
| **A2 — Claude collector** | Provider-grounded Claude snapshots | Correct unknown/stale states; no secret logging |
| **A3 — Codex collector** | Codex usage collection | Quota windows reflect actual source or explicit unsupported status |
| **A4 — Reset intelligence** | Window countdowns and reported bonus/free resets | Absolute timestamps; bonus `0` differs from absent value |
| **A5 — Profiles** | Multiple local profiles and source attribution | Per-profile isolation; no switching CLI login behind user's back |
| **A6 — History** | Optional local usage history and refresh scheduling | Rate-limited polling, no false subscription-total claims |
| **A7 — Read-only contract** | Stable versioned JSON output for other clients | Schema compatibility tests; plugin operates without consumers |

## 9. TDD and failure scenarios

1. Bonus remaining `0` shows `0`; absent bonus field shows `Not reported`.
2. Reset timestamps in the past trigger a stale/refresh indicator, not a negative countdown.
3. Different timezone or DST does not alter absolute reset instants.
4. Provider unavailable, auth expired, rate-limited or malformed response does not crash the popup.
5. Collector fails; previous cache remains labelled stale, not live.
6. Unsupported schema/source never generates a guessed percentage.
7. Pi/OpenCode activity using a shared underlying subscription is not double-counted.
8. Switching displayed profiles never exposes or modifies credentials.
9. Concurrent refresh requests are deduplicated/rate-limited.
10. CLI JSON and UI use the same normalized semantics and redact sensitive fields.

## 10. Rabbit-hole alerts

- **Universal usage API:** Different providers expose different fields, with some undocumented interfaces. Prefer supported sources and isolate adapters.
- **Bonus resets:** Offers vary by account and time; don't assume the Omarchy screenshot applies to everyone.
- **History analytics:** Local token counts are not necessarily account-wide quota usage.
- **TUI polish:** Mouse-first functionality before animations, themes, and graphing.
- **Orq coupling:** Keep this monitor as its own app and publish only a read-only contract.

## 11. Definition of done

A standalone Herdr Agents popup reliably displays verified usage and reset information (including bonus/reset allowances only when reported), handles failures and unknown fields, protects account credentials, and can optionally return a safe, versioned snapshot through a local read-only interface.
