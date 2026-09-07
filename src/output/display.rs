/// ANSI terminal styling and color escape codes.
///
/// These constants can be used to style terminal output.
///
/// # Example
///
/// ```
/// use crate::display::Ansi;
///
/// println!("{}Success!{}", Ansi::GREEN, Ansi::RESET);
/// ```
pub struct Ansi;

#[allow(dead_code)]
impl Ansi {
    /// Reset all terminal formatting.
    pub const RESET: &str = "\x1b[0m";

    /// Bold text.
    pub const BOLD: &str = "\x1b[1m";

    /// Underlined text.
    pub const UNDERLINE: &str = "\x1b[4m";

    /// Italic text.
    pub const ITALIC: &str = "\x1b[3m";

    /// Bright red foreground.
    pub const RED: &str = "\x1b[91m";

    /// Bright yellow foreground.
    pub const YELLOW: &str = "\x1b[93m";

    /// Bright green foreground.
    pub const GREEN: &str = "\x1b[92m";

    /// Bright violet foreground.
    pub const VIOLET: &str = "\x1b[95m";

    /// Bright blue foreground.
    pub const BLUE: &str = "\x1b[94m";

    /// Bright cyan foreground.
    pub const CYAN: &str = "\x1b[96m";

    /// Standard green foreground.
    pub const LIGHT_GREEN: &str = "\x1b[32m";

    /// Dim yellow foreground.
    pub const LIGHT_YELLOW: &str = "\x1b[2;33m";

    /// Standard red foreground.
    pub const LIGHT_RED: &str = "\x1b[31m";
}

/// Print a formatted warning message to stderr.
///
/// The macro displays:
/// - a highlighted warning banner
/// - the file and line number
/// - the formatted message
///
/// # Example
///
/// ```no_run
/// use crate::warning;
///
/// let file = "config.toml";
/// warning!("Failed to load '{}'", file);
/// ```
///
/// Example output:
///
/// ```text
/// =================================================================================
/// ------------------------------------ WARNING ------------------------------------
/// =================================================================================
/// [src/main.rs:42] Failed to load 'config.toml'
/// ```
#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => {{
        use $crate::display::Ansi;

        eprintln!();
        let head = "=".repeat(81);
        let pre = "-".repeat(36);

        eprintln!(
            "{yellow}{head}{reset}",
            head = head,
            yellow = Ansi::YELLOW,
            reset = Ansi::RESET
        );

        eprintln!(
            "{bold}{yellow}{pre} WARNING {pre}{reset}",
            bold = Ansi::BOLD,
            yellow = Ansi::YELLOW,
            reset = Ansi::RESET,
            pre = pre
        );

        eprintln!(
            "{yellow}{head}{reset}",
            head = head,
            yellow = Ansi::YELLOW,
            reset = Ansi::RESET
        );

        eprintln!(
            "{yellow}[{}:{}] {}{reset}",
            file!(),
            line!(),
            format!($($arg)*),
            yellow = Ansi::YELLOW,
            reset = Ansi::RESET
        );

        eprintln!();
    }};
}

/// Print a formatted error message and terminate the process.
///
/// The macro displays:
/// - a highlighted error banner
/// - the file and line number
/// - the formatted message
///
/// After printing the message, the process exits with status code `1`.
///
/// # Example
///
/// ```no_run
/// use crate::error;
///
/// error!("Fatal initialization failure");
/// ```
///
/// Example output:
///
/// ```text
/// =================================================================================
/// ------------------------------------- ERROR -------------------------------------
/// =================================================================================
/// [src/main.rs:15] Fatal initialization failure
/// ```
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {{
        use $crate::display::Ansi;

        eprintln!();
        let head = "=".repeat(81);
        let pre = "-".repeat(36);

        eprintln!(
            "{red}{head}{reset}",
            head = head,
            red = Ansi::RED,
            reset = Ansi::RESET
        );

        eprintln!(
            "{bold}{red}{pre} ERROR {pre}{reset}",
            bold = Ansi::BOLD,
            red = Ansi::RED,
            reset = Ansi::RESET,
            pre = pre
        );

        eprintln!(
            "{red}{head}{reset}",
            head = head,
            red = Ansi::RED,
            reset = Ansi::RESET
        );

        eprintln!(
            "{red}[{}:{}] {}{reset}",
            file!(),
            line!(),
            format!($($arg)*),
            red = Ansi::RED,
            reset = Ansi::RESET
        );

        eprintln!();
        std::process::exit(1);
    }};
}

pub(crate) use warning;
