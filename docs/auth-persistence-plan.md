# Persist Bandcamp tokens + re-authorization flow

## Context

Today `main.rs` prompts for username/password whenever `collection.jsonl` is missing, and once it exists the app never talks to Bandcamp again (so new purchases and possibly-expiring `hq_audio_url`s are never refreshed). The login response already contains `expires_in` and `refresh_token`, but `LoginResponse` (`src/bandcamp.rs`) discards them.

Goal:
1. Persist the access token, refresh token and expiry to `~/.local/state/rusty-piano/auth.json` (0600), so startup only needs a login/refresh when the access token is no longer valid.
2. Implement the `grant_type=refresh_token` flow so credentials are only ever typed once and never stored.
3. Per the user's choice, **re-sync the collection on every startup** using the token, falling back to the cached `collection.jsonl` when offline. (`collection.jsonl` stays in the working directory for now.)

The refresh grant was confirmed in step 0: it works against `/oauth_token` (not `/oauth_login`). See the step 0 findings.

## Step 0 — Verify the refresh grant (throwaway probe)

Add a temporary `examples/auth_probe.rs` (not committed) that logs in with the password grant, prints `expires_in`, then calls `send_oauth_request` with:

```
grant_type=refresh_token, refresh_token=<token>, client_id=134, client_secret=<same>
```

Record: whether it returns 200 with a new `access_token`; whether `refresh_token` is rotated or omitted; whether the 418 Dm challenge (and/or a 451 Pow step) occurs; the `expires_in` value; and what `GET /api/collectionsync/1/collection` returns for a bogus bearer token (expect 401). Adjust the steps below if any assumption is wrong. If refresh is unsupported, everything else still works: an expired token just falls back to the password prompt (credentials are still never stored).

### Findings (2026-11-09) — done

- **`/oauth_login` rejects the refresh grant.** It returns HTTP 200 with `{"error": true, "ok": false, "exception": "... Invalid grant type \"refresh_token\""}`, both with and without a Dm signature. The grant type is checked before the challenge.
- **`POST https://bandcamp.com/oauth_token` accepts it.** This is the developer-API endpoint documented at <https://bandcamp.com/developer#refresh_tokens>. It works with the Android `client_id=134`/secret and a refresh token from `/oauth_login`. It needs **one plain form POST, with no 418 Dm challenge and no Pow step**, and returns the same shape as login: `{access_token, expires_in: 3600, ok: true, refresh_token, token_type: "bearer"}`.
- **The refresh token rotates** on every refresh, but **the previous one stays usable**: refreshing again with the original token succeeded. So a crash between refreshing and saving `auth.json` isn't fatal.
- **The access token came back unchanged** when refreshed within its lifetime, and the original stayed valid. It's unknown whether refresh extends that token's server-side expiry or issues a new token only once the old one expires. Rely on the 401 → refresh → retry path rather than trusting `expires_at` alone.
- **`expires_in` is 3600 s**, matching the developer docs ("Access tokens expire in one hour").
- **A bad or expired bearer token on the collection endpoint gets `401`** with an empty body.
- **There is a second error shape:** `{"error": true, "exception": "..."}`, alongside the existing `{"error": "<code>", "error_description": "..."}`. `LoginErrorResponse` can't parse the boolean one.
- The developer API is officially for labels and merch partners. Using the Android client credentials against `/oauth_token` is outside its intended use, and the user has accepted that.

## Step 1 — Make the OAuth helpers return errors instead of panicking (`src/bandcamp.rs`)

A rejected refresh token must lead to a password prompt, not a crash.

- `send_oauth_request`: replace both `assert_eq!` status checks and the header `.unwrap()` with `anyhow::bail!` / `.context(...)`.
- `build_oauth_request`: `.unwrap()` → `.context("oauth request body is not buffered")?`.
- `solve_dm_challenge`: `.nth(i).unwrap()` → `.context(...)`, `assert_eq!(1, algorithm)` → `bail!("unsupported X-Bandcamp-Dm algorithm {algorithm}")`.
- Hoist `client_id`/`client_secret` into `CLIENT_ID` / `CLIENT_SECRET` constants next to `OAUTH_LOGIN_URL`, and add `OAUTH_TOKEN_URL = "https://bandcamp.com/oauth_token"`.
- Make `send_oauth_request` private again; it was made `pub` only for the probe.

## Step 2 — Token model and refresh grant (`src/bandcamp.rs`)

- Rename `LoginResponse` → `TokenResponse { access_token, expires_in: u64, refresh_token: Option<String> }`. `Option` so a refresh response that omits it keeps the old token.
- Extract the existing `contains("error")` → `LoginErrorResponse` parsing in `login` into `parse_token_response(body: &str) -> Result<TokenResponse>`, shared by both grants. It must handle both error shapes from step 0: `{"error": "<code>", "error_description"}` and `{"error": true, "exception"}`. Check `ok == false` or the presence of `error` via a `serde_json::Value`, then map either shape to an `AuthError` with a readable message.
- Add `fn refresh_grant(client: &Client, refresh_token: &str) -> Result<TokenResponse>`. It sends a **single** form POST to `OAUTH_TOKEN_URL` with `grant_type=refresh_token`, `refresh_token`, `client_id`, `client_secret` and no Dm header, then calls `parse_token_response`. It does not use `send_oauth_request`, whose challenge round-trip only applies to `/oauth_login`.
- Always persist the rotated `refresh_token` from each refresh response.
- New public, serializable type:
  ```rust
  #[derive(Clone, Debug, Serialize, Deserialize)]
  pub struct AuthTokens { pub access_token: String, pub refresh_token: String, pub expires_at: u64 /* unix secs */ }
  ```
  with `fn from_response(resp, previous_refresh: Option<&str>, now: u64)` and `fn is_expired(&self, now: u64) -> bool` (60 s safety margin). `now` is passed in so the logic is unit-testable.

## Step 3 — Restructure `BandCampClient` (`src/bandcamp.rs`)

- Field `access_token: String` → `tokens: AuthTokens`.
- Move the header/cookie `Client` setup out of `new` into `fn build_http_client() -> Result<Client>`.
- Constructors:
  - `pub fn login(username, password) -> Result<Self>` (replaces `new`).
  - `pub fn from_tokens(tokens: AuthTokens) -> Result<Self>`: if `is_expired`, call `refresh()` immediately; any error propagates so the caller can fall back to a password prompt.
- `fn refresh(&mut self) -> Result<()>` calls `refresh_grant` and replaces `self.tokens`.
- `pub fn tokens(&self) -> &AuthTokens` so `main` can persist them after use.
- `get_collection` / `get_entire_collection` take `&mut self`. On a `401 UNAUTHORIZED`, refresh once and retry the page (this covers a server-side revocation before the local expiry). Also check `error_for_status()` before `.json()` so HTTP errors aren't reported as JSON decode errors.

## Step 4 — Token persistence (new `src/auth_store.rs`, add to `src/lib.rs`)

- Add `dirs` to `Cargo.toml`.
- `pub fn default_path() -> Option<PathBuf>`: `dirs::state_dir().or_else(dirs::data_local_dir)` joined with `rusty-piano/auth.json`.
- `pub fn load(path: &Path) -> Result<Option<AuthTokens>>`: a missing file gives `Ok(None)`. A corrupt file is also treated as `None`, with a printed warning.
- `pub fn save(path: &Path, tokens: &AuthTokens) -> Result<()>`: `create_dir_all` the parent, write a sibling temp file opened with `OpenOptionsExt::mode(0o600)` (`#[cfg(unix)]`), then `fs::rename` over the target for an atomic replace.
- `pub fn clear(path: &Path) -> Result<()>`: delete on a definitive auth rejection so a dead token isn't retried every start.

## Step 5 — Startup flow (`src/main.rs`)

Replace the `match File::open(...)` / `login_and_cache_collection` logic with `load_collection(collection_path, auth_path)`:

1. `auth_store::load` → if `Some`, `BandCampClient::from_tokens`. On error, print it and `auth_store::clear`.
2. If there's no client, run the existing username/password loop (lifted from `login_and_cache_collection`). If `collection.jsonl` exists, tell the user an **empty username skips login and uses the cached collection** (this covers offline use and stops the loop spinning when the network is down).
3. With a client: `get_entire_collection`, write `collection.jsonl` via `write_lines_to_file`, then `auth_store::save(client.tokens())` (this also captures a mid-sync refresh).
4. If the sync fails and a cached `collection.jsonl` exists, print a warning and fall back to `read_lines_from_file`; if no cache exists, return the error.

The ratatui setup after this is unchanged.

## Step 6 — Housekeeping

- README: tick "Re-auth flow" and "Find a way to store/re-use auth-token", and add a short note on where `auth.json` lives and how to log out (delete it).
- Delete `examples/auth_probe.rs`.

## Critical files

- `src/bandcamp.rs`: reuses `send_oauth_request`, `build_oauth_request`, `solve_dm_challenge`, and `LoginErrorResponse`.
- `src/auth_store.rs` (new), `src/lib.rs`, `src/main.rs`, `Cargo.toml`, `README.md`.
- Reuse `json_l::{read_lines_from_file, write_lines_to_file}` for the collection cache.

## Verification

- **Unit tests:**
  - `AuthTokens::is_expired` at the boundaries and with the margin.
  - `from_response` keeps the previous refresh token when the response omits it.
  - `parse_token_response` with the two example error bodies already in the code comments, plus a success body.
  - `auth_store` save/load round trip in a temp dir, asserting file mode `0o600`, and a missing file giving `None`.
- `cargo clippy --all-targets` adds no new warnings; `cargo test` passes.
- **Manual, end to end** (`cargo run`):
  1. Fresh run with no `auth.json`: prompts for credentials, syncs, and creates `~/.local/state/rusty-piano/auth.json` with `-rw-------`.
  2. Second run: no prompt, collection re-synced.
  3. Edit `expires_at` to `0` in `auth.json`: no prompt, a refresh happens, and the file is updated with a new expiry.
  4. Corrupt the `refresh_token` and set `expires_at` to `0`: the error is printed, `auth.json` is cleared, and the password prompt appears.
  5. Offline with a cache present: an empty username (or a failed sync) falls back to the cached collection and the UI starts.
