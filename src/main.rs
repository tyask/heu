use clap::Parser;
use std::fs;
use std::path::Path;

use cargo_heu::{Config, Heu};

#[derive(Parser)]
#[command(name = "cargo-huu", about = "Test harness for heuristic programming contests")]
struct Cli {
    /// When invoked as `cargo heu`, the first arg is "heu" — consumed by clap as subcommand.
    #[command(subcommand)]
    sub: Option<Sub>,
}

#[derive(Parser)]
enum Sub {
    /// Run heuristic contest test harness
    Heu(Args),
}

#[derive(Parser)]
pub struct Args {
    /// Test cases (e.g. 0 1 3-5)
    cases: Vec<String>,

    /// Config file path (default: ./heu.toml)
    #[arg(short = 'f', long = "config")]
    config: Option<String>,

    /// Number of parallel threads
    #[arg(short = 'j', long = "threads")]
    threads: Option<usize>,

    /// Run without evaluation (skip visualizer scoring)
    #[arg(short = 'n', long = "no-evaluate")]
    no_evaluate: bool,

    /// Use tester.exe for interactive problems
    #[arg(short = 't', long = "tester")]
    use_tester: bool,
}

fn load_config(config_path: Option<&str>) -> Config {
    let path = config_path.unwrap_or("./heu.toml");

    if Path::new(path).exists() {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read config file '{}': {}", path, e));
        toml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse config file '{}': {}", path, e))
    } else if config_path.is_some() {
        panic!("Config file '{}' not found", path);
    } else {
        let config = Config::default_config();
        let toml_str = config.generate_toml_with_comments();
        fs::write(path, &toml_str)
            .unwrap_or_else(|e| panic!("Failed to write default config '{}': {}", path, e));
        eprintln!("Generated default config: {}", path);
        config
    }
}

fn main() {
    let cli = Cli::parse();

    let args = match cli.sub {
        Some(Sub::Heu(a)) => a,
        None => Args::parse_from(std::env::args().skip(0)),
    };

    let mut config = load_config(args.config.as_deref());

    // CLI引数でconfigのフィールドを上書き
    if !args.cases.is_empty() {
        config.test.cases = args.cases.join(" ");
    }
    if let Some(threads) = args.threads {
        config.test.threads = threads;
    }
    if args.no_evaluate {
        config.test.no_evaluate = true;
    }
    if args.use_tester {
        config.test.use_tester = true;
    }

    let heu = Heu::new(config);

    if let Err(e) = heu.execute() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
