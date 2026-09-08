use crate::squire;

/// Gets the default host from the `remote_host` environment variable.
pub fn default_host() -> String {
    squire::misc::get_env_var("remote_host", None)
}

/// Gets the default username from the `remote_user` environment variable.
pub fn default_username() -> String {
    squire::misc::get_env_var("remote_user", None)
}

/// Gets the default remote path from the `remote_path` environment variable.
pub fn default_path() -> String {
    squire::misc::get_env_var("remote_path", None)
}

/// Gets the default rsync timeout from `rsync_timeout` environment variable.
pub fn default_timeout() -> u8 {
    squire::misc::get_env_var("rsync_timeout", None)
        .parse::<u8>()
        .unwrap_or(3)
}

/// Gets the default save path from the `save_path` environment variable.
pub fn default_save_path() -> String {
    String::new()
}

/// Determines whether files should be deleted after copying.
///
/// This value is read from the `delete_after_copy` environment variable.
/// If the variable is missing or cannot be parsed as a boolean,
/// it defaults to `false`, since this is called during run-time.
pub fn default_delete_after_copy() -> bool {
    squire::misc::get_env_var("delete_after_copy", Some("false"))
        .parse::<bool>()
        .unwrap_or(false)
}
