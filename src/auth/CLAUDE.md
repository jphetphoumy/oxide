# auth — Authentication Module

OAuth device flow authentication against WorkOS, with secure token persistence and auto-refresh.

## Files

| File | Purpose |
|------|---------|
| `mod.rs` | Public API: `logout()`, `status()` |
| `device_flow.rs` | WorkOS OAuth device code flow (`login()`, `request_device_code()`, `poll_for_token()`) |
| `token_storage.rs` | System keyring abstraction via `keyring` crate (access token, refresh token, workspace ID, region) |
| `token_refresh.rs` | Token expiry check (`is_token_expired()`) and auto-refresh (`get_valid_token()`) |
| `jwt.rs` | JWT payload decoding without signature verification (metadata only, not for auth decisions) |
| `workspace_selection.rs` | Workspace prompt after login, fetches from `/api/v1/me` endpoint |

## Auth Flow

1. `oxide login` calls `device_flow::login()`
2. Requests device code from WorkOS, opens browser for user consent
3. Polls WorkOS until user authorizes or timeout
4. Extracts region from JWT claim `https://dust.tt/region`
5. Saves access token, refresh token, and region to system keyring
6. Prompts user to select workspace (or auto-selects if only one)

## Key Patterns

- **Token access**: Always go through `token_refresh::get_valid_token()` — it handles expiry + refresh transparently
- **Keyring abstraction**: `token_storage` uses a `KeyStore` trait with `SystemKeyStore` for prod and `MemoryKeyStore` for tests (swappable via `set_store_for_tests()`)
- **Test isolation**: Tests that touch `token_storage` must acquire a shared `Mutex` (`test_lock()`) since the store is global state
- **JWT decoding**: `jwt::decode_payload()` is intentionally unverified — only used for non-security-critical metadata (region, expiry)
- **WorkOS client ID**: Defaults to `DEFAULT_WORKOS_CLIENT_ID`, overridable via `OXIDE_WORKOS_CLIENT_ID` env var

## Constants

- WorkOS authorize URL: `https://api.workos.com/user_management/authorize/device`
- WorkOS authenticate URL: `https://api.workos.com/user_management/authenticate`
- Default region: `us-central1`
- Token expiry skew: 30 seconds (refresh slightly before actual expiry)
- Keyring service name: `oxide`
