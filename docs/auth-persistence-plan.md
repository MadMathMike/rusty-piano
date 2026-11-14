# Persist Bandcamp tokens + re-authorization flow

## Context

Today `main.rs` prompts for username/password whenever `collection.jsonl` is missing, and once it exists the app never talks to Bandcamp again (so new purchases and possibly-expiring `hq_audio_url`s are never refreshed). The login response already contains `expires_in` and `refresh_token`, but `LoginResponse` (`src/bandcamp.rs`) discards them.

Goal:
1. Persist **only the refresh token**, in the OS credential store via the [`keyring`](https://crates.io/crates/keyring) crate: Keychain on macOS, Credential Manager on Windows, and Secret Service (GNOME Keyring / KWallet) on Linux. It is never written to a plain file. Every startup exchanges it for a fresh access token, which is kept in memory only.
2. Implement the `grant_type=refresh_token` flow so credentials are only ever typed once and never stored.
3. Per the user's choice, **re-sync the collection on every startup** using the token, falling back to the cached `collection.jsonl` when offline. (`collection.jsonl` stays in the working directory for now.)

The refresh grant was confirmed in step 0: it works against `/oauth_token` (not `/oauth_login`). See the step 0 findings.

### Why only the refresh token

The refresh POST sends just `grant_type`, `refresh_token`, `client_id` and `client_secret`. It needs no access token, bearer header or cookies. It works whether or not the previous access token is still valid: within its lifetime it returns the same access token, and after it expires it returns a new one (assumed; see the open questions). Storing the access token and `expires_at` would only save one cheap, unchallenged POST per startup, and that startup already does a full collection sync over the network. Refreshing unconditionally means no expiry bookkeeping, no clock-skew concerns, one fewer secret on disk, and no "trust the stored token, then get a 401" path at startup.

## Step 0 — Verify the refresh grant (throwaway probe)

Add a temporary `examples/auth_probe.rs` (not committed) that logs in with the password grant, prints `expires_in`, then calls `send_oauth_request` with:

```
grant_type=refresh_token, refresh_token=<token>, client_id=134, client_secret=<same>
```

Record: whether it returns 200 with a new `access_token`; whether `refresh_token` is rotated or omitted; whether the 418 Dm challenge (and/or a 451 Pow step) occurs; the `expires_in` value; and what `GET /api/collectionsync/1/collection` returns for a bogus bearer token (expect 401). Adjust the steps below if any assumption is wrong. If refresh is unsupported, everything else still works: a rejected refresh just falls back to the password prompt (credentials are still never stored).

### Findings (2026-11-09) — done

- **`/oauth_login` rejects the refresh grant.** It returns HTTP 200 with `{"error": true, "ok": false, "exception": "... Invalid grant type \"refresh_token\""}`, both with and without a Dm signature. The grant type is checked before the challenge.
- **`POST https://bandcamp.com/oauth_token` accepts it.** This is the developer-API endpoint documented at <https://bandcamp.com/developer#refresh_tokens>. It works with the Android `client_id=134`/secret and a refresh token from `/oauth_login`. It needs **one plain form POST, with no 418 Dm challenge and no Pow step**, and returns the same shape as login: `{access_token, expires_in: 3600, ok: true, refresh_token, token_type: "bearer"}`.
- **The refresh token rotates** on every refresh, but **the previous one stays usable**: refreshing again with the original token succeeded. So a crash between refreshing and saving the new token to the keyring isn't fatal.
- **The access token came back unchanged** when refreshed within its lifetime, and the original stayed valid. It's unknown whether refresh extends that token's server-side expiry or issues a new token only once the old one expires. This doesn't matter when refreshing on every startup: either way the returned token is valid.
- **`expires_in` is 3600 s**, matching the developer docs ("Access tokens expire in one hour").
- **A bad or expired bearer token on the collection endpoint gets `401`** with an empty body.
- **There is a second error shape:** `{"error": true, "exception": "..."}`, alongside the existing `{"error": "<code>", "error_description": "..."}`. `LoginErrorResponse` can't parse the boolean one.
- The developer API is officially for labels and merch partners. Using the Android client credentials against `/oauth_token` is outside its intended use, and the user has accepted that.

### Open questions (not covered by the probe)

The probe ran within minutes of logging in, so these are untested. The plan **assumes the first holds**.

- Does a refresh still succeed once the access token has actually expired (after 3600 s)?
- How long does the refresh token itself last (days, weeks, indefinitely)?

If either turns out badly, the app still works: the refresh is rejected, the keyring entry is deleted and the user is prompted for credentials. Only the "type credentials once" goal degrades.

## Step 1 — Make the OAuth helpers return errors instead of panicking (`src/bandcamp.rs`)

A rejected refresh token must lead to a password prompt, not a crash.

- `send_oauth_request`: replace both `assert_eq!` status checks and the header `.unwrap()` with `anyhow::bail!` / `.context(...)`.
- `build_oauth_request`: `.unwrap()` → `.context("oauth request body is not buffered")?`.
- `solve_dm_challenge`: `.nth(i).unwrap()` → `.context(...)`, `assert_eq!(1, algorithm)` → `bail!("unsupported X-Bandcamp-Dm algorithm {algorithm}")`.
- Hoist `client_id`/`client_secret` into `CLIENT_ID` / `CLIENT_SECRET` constants next to `OAUTH_LOGIN_URL`, and add `OAUTH_TOKEN_URL = "https://bandcamp.com/oauth_token"`.
- Make `send_oauth_request` private again; it was made `pub` only for the probe.

## Step 2 — Token model and refresh grant (`src/bandcamp.rs`)

- Rename `LoginResponse` → `TokenResponse { access_token: String, refresh_token: Option<String> }`. `Option` so a refresh response that omits it keeps the old token. `expires_in` isn't needed and can stay undeserialized.
- Extract the existing `contains("error")` → `LoginErrorResponse` parsing in `login` into `parse_token_response(body: &str) -> Result<TokenResponse>`, shared by both grants. It must handle both error shapes from step 0: `{"error": "<code>", "error_description"}` and `{"error": true, "exception"}`. Check `ok == false` or the presence of `error` via a `serde_json::Value`, then map either shape to an `AuthError` with a readable message.
- Add `fn refresh_grant(client: &Client, refresh_token: &str) -> Result<TokenResponse>`. It sends a **single** form POST to `OAUTH_TOKEN_URL` with `grant_type=refresh_token`, `refresh_token`, `client_id`, `client_secret` and no Dm header, then calls `parse_token_response`. It does not use `send_oauth_request`, whose challenge round-trip only applies to `/oauth_login`.
- There is no `AuthTokens` type, `expires_at` or `is_expired`. The persisted state is just the refresh token string.

## Step 3 — Restructure `BandCampClient` (`src/bandcamp.rs`)

- Fields: `access_token: String` (in memory only) and `refresh_token: String`.
- Move the header/cookie `Client` setup out of `new` into `fn build_http_client() -> Result<Client>`.
- Constructors:
  - `pub fn login(username, password) -> Result<Self>` (replaces `new`). Fails if the login response has no `refresh_token`.
  - `pub fn from_refresh_token(refresh_token: &str) -> Result<Self>`: always calls `refresh_grant`. Any error propagates so the caller can fall back to a password prompt.
- `fn refresh(&mut self) -> Result<()>` calls `refresh_grant` and replaces both tokens, keeping the old refresh token if the response omits one.
- `pub fn refresh_token(&self) -> &str` so `main` can persist the current (possibly rotated) token.
- `get_collection` / `get_entire_collection` take `&mut self`. On a `401 UNAUTHORIZED`, refresh once and retry the page. This only matters if a sync outlives the one-hour access token or the token is revoked server-side, but it's cheap. Also check `error_for_status()` before `.json()` so HTTP errors aren't reported as JSON decode errors.

## Step 4 — Token persistence via keyring (new `src/token_store.rs`, add to `src/lib.rs`)

### Why keyring

The file-based alternative (a JSON file under a platform data directory) has no clean cross-platform location (`state_dir` is Linux-only) and stores the token in plaintext, protected only by `0600` on Unix. The OS credential store encrypts the token at rest, ties it to the user's login, and looks the same from the app on all three platforms.

### Dependency

- Add `keyring = "4"` with default features. The default `v1` feature selects the native store per platform (`apple-native-keyring-store`, `windows-native-keyring-store`, `zbus-secret-service-keyring-store` with pure-Rust crypto) and exposes the simple `keyring::Entry` API. No async runtime or system `libdbus`/OpenSSL is needed.
- Don't use the `cli` feature or depend on `keyring-core` directly. Those are for apps that pick their own stores; we just want "the native one".

### API (`keyring` 4.x, `v1` feature)

- `keyring::Entry::new(service, user) -> keyring::Result<Entry>`. The first call initializes the platform store; if that fails, it returns `Error::NoDefaultStore` (`Entry::store_status()` has the underlying cause).
- `entry.get_password()`, `entry.set_password(&str)`, `entry.delete_credential()`.
- `keyring::Error::NoEntry` means "nothing stored". Everything else (`NoStorageAccess`, `PlatformFailure`, `NoDefaultStore`, ...) means the store itself is unavailable or failed.

### Module

- Constants: `SERVICE = "rusty-piano"`, `USER = "bandcamp-refresh-token"`. There is a fixed user name because the Bandcamp username is deliberately not stored. (These are the names you'll see in Keychain Access / Credential Manager / Seahorse.)
- `fn entry() -> Result<keyring::Entry>`: `Entry::new(SERVICE, USER)` with `.context("OS credential store unavailable")`.
- `pub fn load() -> Result<Option<String>>`: `Err(NoEntry)` → `Ok(None)`. Other errors propagate. An empty stored string is treated as `None`.
- `pub fn save(refresh_token: &str) -> Result<()>`: `set_password`. This overwrites any existing value, so no temp file or rename is needed.
- `pub fn clear() -> Result<()>`: `delete_credential`, treating `NoEntry` as success. Called on a definitive auth rejection so a dead token isn't retried every start.
- The on-disk JSON format, `StoredAuth`, file modes and directory handling are all gone. If more fields ever need storing, put a small JSON blob in the password (Windows Credential Manager caps it at about 2.5 KB, far more than a token needs).

### When the keyring is unavailable

For example, a headless Linux box with no Secret Service daemon, a locked collection the user declines to unlock, or Keychain access denied. This must not stop the app:

- `load` errors: print a one-line warning ("couldn't read saved login from the OS keyring: ...; you'll be asked to log in") and continue as if nothing was stored.
- `save` / `clear` errors: print a warning and continue. The session works, the user just gets prompted again next time.
- There is **no plaintext-file fallback**. That would quietly undo the point of using keyring.

### Platform notes

- **macOS:** Keychain ACLs are tied to the binary's code signature. Unsigned `cargo build` output changes on every rebuild, so during development macOS may ask "rusty-piano wants to use your confidential information" after each rebuild. Clicking "Always Allow" is fine; this goes away for a stable, signed release build.
- **Linux:** this needs a running Secret Service provider (GNOME Keyring or KWallet), which normal desktop sessions on Fedora have. If the login collection is locked, the provider may show an unlock prompt.
- **Windows:** this stores a generic credential in Credential Manager, scoped to the current user. There's nothing special to handle.

## Step 5 — Startup flow (`src/main.rs`)

Replace the `match File::open(...)` / `login_and_cache_collection` logic (and its TODO comments) with `load_collection(collection_path)`:

1. `token_store::load` (warning on error, as above) → if `Some`, `BandCampClient::from_refresh_token`.
   - On success, **immediately** `token_store::save(client.refresh_token())`, before syncing, so the rotated token is kept even if the sync fails.
   - On an auth rejection, print it and `token_store::clear`.
2. If there's no client, run the existing username/password loop (lifted from `login_and_cache_collection`). On a successful login, `token_store::save(client.refresh_token())`. If `collection.jsonl` exists, tell the user an **empty username skips login and uses the cached collection** (this covers offline use and stops the loop spinning when the network is down).
3. With a client: `get_entire_collection`, write `collection.jsonl` via `write_lines_buf`, then `token_store::save(client.refresh_token())` again if it changed (captures a mid-sync refresh).
4. If the sync fails and a cached `collection.jsonl` exists, print a warning and fall back to `read_lines_buf`; if no cache exists, return the error.

A network failure during the startup refresh (as opposed to a rejection) should not clear the stored token. Only clear it when Bandcamp returns an auth error from `parse_token_response`; for transport errors, skip straight to the cached-collection fallback. Keyring errors never clear anything and never abort startup.

The ratatui setup after this is unchanged.

## Step 6 — Housekeeping

- README: tick "Re-auth flow" and "Find a way to store/re-use auth-token", and add a short note that the refresh token is kept in the OS keyring (service `rusty-piano`, account `bandcamp-refresh-token`) and that logging out means deleting that entry (Keychain Access / Credential Manager / Seahorse, or `secret-tool clear service rusty-piano` on Linux). A `--logout` flag that calls `token_store::clear` is an easy follow-up, but out of scope here.
- Delete `examples/auth_probe.rs`.

## Critical files

- `src/bandcamp.rs`: reuses `send_oauth_request`, `build_oauth_request`, `solve_dm_challenge`, and `LoginErrorResponse`.
- `src/token_store.rs` (new), `src/lib.rs`, `src/main.rs`, `Cargo.toml`, `README.md`.
- Reuse `json_l::{read_lines_buf, write_lines_buf}` for the collection cache.

## Verification

- **Unit tests:**
  - `parse_token_response` with the two error shapes from step 0 (and the example error bodies already in the code comments), plus a success body with and without `refresh_token`.
  - `BandCampClient::refresh` keeps the previous refresh token when the response omits it (test the token-merging logic as a pure function if a live client is awkward).
  - `token_store`: keep the `NoEntry` → `None` / `clear`-is-idempotent mapping in small pure helpers (e.g. `fn none_if_missing(r: keyring::Result<String>) -> Result<Option<String>>`) and unit-test those with constructed `keyring::Error` values. These don't touch the real keyring.
  - One `#[ignore]` test that does save → load → clear → load against the real OS keyring, under a distinct `USER` (e.g. `"test-refresh-token"`) so it can't clobber a real login. Run it manually with `cargo test -- --ignored`. It's ignored by default because CI and headless machines usually have no Secret Service.
- `cargo clippy --all-targets` adds no new warnings; `cargo test` passes.
- **Manual, end to end** (`cargo run`):
  1. Fresh run with no keyring entry: prompts for credentials, syncs, and creates the entry. Check with `secret-tool lookup service rusty-piano username bandcamp-refresh-token` or Seahorse; it holds only a refresh token.
  2. Second run within the hour: no prompt, a refresh happens, the collection is re-synced, and the keyring entry holds the rotated token.
  3. Run again more than an hour later: no prompt (this confirms the open question about refreshing after expiry).
  4. Overwrite the entry with junk (`secret-tool store --label=x service rusty-piano username bandcamp-refresh-token`): the auth error is printed, the entry is deleted, and the password prompt appears.
  5. Offline with a cache present: the refresh fails with a transport error, the keyring entry is **not** deleted, and an empty username (or the failed sync) falls back to the cached collection and the UI starts.
  6. Keyring unavailable (e.g. run with `DBUS_SESSION_BUS_ADDRESS=` unset/bogus): a warning is printed, the password prompt appears, login and sync work, and saving prints a warning instead of failing.
