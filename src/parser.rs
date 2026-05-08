use crate::constant;

#[derive(Debug, Clone)]
pub struct Arguments {
    pub env_file: String,
    pub read_db: bool,
}

/// Parses and returns the command-line arguments.
///
/// # Returns
///
/// A String notion of the argument, `env_file` if present.
pub fn arguments(metadata: &constant::MetaData) -> Arguments {
    let args: Vec<String> = std::env::args().collect();

    let mut version = false;
    let mut env_file = String::new();
    let mut read_db = false;

    // Loop through the command-line arguments and parse them.
    let mut i = 1; // Start from the second argument (args[0] is the program name).
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                let helper = "RuTorrent takes the arguments, --env_file and --version/-v\n\n\
                --read_db: Boolean flag to read database on demand. Defaults to `false`\n\
                --env_file: Custom filename to load the environment variables. Defaults to '.env'\n\
                --version: Get the package version.\n"
                    .to_string();
                println!("Usage: {} [OPTIONS]\n\n{}", args[0], helper);
                std::process::exit(0)
            }
            "-V" | "-v" | "--version" => {
                version = true;
            }
            "--env_file" => {
                i += 1; // Move to the next argument.
                if i < args.len() {
                    env_file = args[i].clone();
                } else {
                    eprintln!("--env_file requires a value.");
                    std::process::exit(1)
                }
            }
            "--read_db" => {
                i += 1; // Move to the next argument.
                if i < args.len() {
                    let read_db_raw = args[i].clone();
                    read_db = read_db_raw.parse::<bool>().unwrap_or_else(|err| {
                        eprintln!("\nStartupError:\n\t{}\n", err);
                        std::process::exit(1)
                    });
                } else {
                    eprintln!("--read_db requires a value.");
                    std::process::exit(1)
                }
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                std::process::exit(1)
            }
        }
        i += 1;
    }
    if version {
        println!("{} {}", &metadata.pkg_name, &metadata.pkg_version);
        std::process::exit(0)
    }
    Arguments { env_file, read_db }
}
