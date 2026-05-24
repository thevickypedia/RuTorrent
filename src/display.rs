#[allow(dead_code)]
pub mod ansi {
    pub const RESET: &str = "\x1b[0m";

    pub const BOLD: &str = "\x1b[1m";
    pub const UNDERLINE: &str = "\x1b[4m";
    pub const ITALIC: &str = "\x1b[3m";

    pub const RED: &str = "\x1b[91m";
    pub const YELLOW: &str = "\x1b[93m";
    pub const GREEN: &str = "\x1b[92m";

    pub const VIOLET: &str = "\x1b[95m";
    pub const BLUE: &str = "\x1b[94m";
    pub const CYAN: &str = "\x1b[96m";
    pub const LIGHT_GREEN: &str = "\x1b[32m";
    pub const LIGHT_YELLOW: &str = "\x1b[2;33m";
    pub const LIGHT_RED: &str = "\x1b[31m";
}
