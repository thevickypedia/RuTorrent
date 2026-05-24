pub struct Ansi;

#[allow(dead_code)]
impl Ansi {
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

#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => {{
        use crate::display::Ansi;

        eprintln!();
        let head = "=".repeat(81);
        let pre = "-".repeat(36);

        eprintln!("{yellow}{head}{reset}",
            head = head,
            yellow = Ansi::YELLOW,
            reset = Ansi::RESET
        );

        eprintln!("{bold}{yellow}{pre} WARNING {pre}{reset}",
            bold = Ansi::BOLD,
            yellow = Ansi::YELLOW,
            reset = Ansi::RESET,
            pre = pre
        );

        eprintln!("{yellow}{head}{reset}",
            head = head,
            yellow = Ansi::YELLOW,
            reset = Ansi::RESET
        );

        eprintln!("{yellow}[{}:{}] {}{reset}",
            file!(),
            line!(),
            format!($($arg)*),
            yellow = Ansi::YELLOW,
            reset = Ansi::RESET
        );

        eprintln!();
    }};
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {{
        use crate::display::Ansi;

        eprintln!();
        let head = "=".repeat(81);
        let pre = "-".repeat(36);

        eprintln!("{red}{head}{reset}",
            head = head,
            red = Ansi::RED,
            reset = Ansi::RESET
        );

        eprintln!("{bold}{red}{pre} ERROR {pre}{reset}",
            bold = Ansi::BOLD,
            red = Ansi::RED,
            reset = Ansi::RESET,
            pre = pre
        );

        eprintln!("{red}{head}{reset}",
            head = head,
            red = Ansi::RED,
            reset = Ansi::RESET
        );

        eprintln!("{red}[{}:{}] {}{reset}",
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
