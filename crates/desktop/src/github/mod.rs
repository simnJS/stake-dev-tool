pub mod api;
pub mod auth;

pub use auth::{AuthState, DeviceCode, GithubUser};

/// OAuth App client ID. Device Flow doesn't use the client secret, so this
/// string is public and safe to ship in source — GitHub CLI, VS Code, and
/// countless other OSS desktop apps hardcode theirs the same way.
pub const OAUTH_CLIENT_ID: &str = "Ov23liEQ8WQsoUmRg6wg";

/// Scopes needed:
///   - `repo` — read/write private repos (team metadata + Release assets)
///   - `read:user` — fetch authenticated user profile for display
///   - `delete_repo` — permanently delete a team repo when the owner chooses
///     to disband the team
pub const OAUTH_SCOPES: &str = "repo read:user delete_repo";
