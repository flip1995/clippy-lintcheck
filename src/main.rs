use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    str::FromStr,
};
use structopt::StructOpt;
use tempfile::NamedTempFile;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "clippy-lintcheck",
    about = "Run the clippy-lintcheck tool on the configurations"
)]
struct Opt {
    /// Check all configuration files. Available options: "all", "passes", "integration", "ci"
    #[structopt(long, required = true)]
    mode: Mode,
}

#[derive(Debug, StructOpt)]
enum Mode {
    All,
    Passes,
    Integration,
    CI,
}

impl FromStr for Mode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" => Ok(Self::All),
            "passes" => Ok(Self::Passes),
            "integration" => Ok(Self::Integration),
            "ci" => Ok(Self::CI),
            err => Err(format!("Invalid option {}", err)),
        }
    }
}

fn check(clippy_path: &Path, config: &Path, output: Option<&str>) {
    let lintcheck_output = Command::new("cargo")
        .arg("dev-lintcheck")
        .env("LINTCHECK_TOML", config)
        .current_dir(clippy_path)
        .output()
        .expect("couldn't execute lintcheck tool");
    if !lintcheck_output.status.success() {
        panic!(
            "cargo dev-lintcheck exited with {}\nstderr:\n{:?}",
            lintcheck_output.status,
            String::from_utf8_lossy(&lintcheck_output.stderr),
        );
    }
    println!(
        "lintcheck stdout: {}",
        String::from_utf8_lossy(&lintcheck_output.stdout)
    );
    fs::copy(
        clippy_path.join(format!(
            "lintcheck-logs/{}_logs.txt",
            config.file_stem().unwrap().to_string_lossy()
        )),
        format!(
            "logs/{}_logs.txt",
            output.unwrap_or(&config.file_stem().unwrap().to_string_lossy())
        ),
    )
    .expect("couldn't copy log file");
}

fn check_integration(clippy_path: &Path) {
    check(
        clippy_path,
        &PathBuf::from("../config/integration.toml"),
        None,
    );
    let log_integration =
        fs::read_to_string("logs/integration_logs.txt").expect("couldn't read log file");
    assert!(log_integration.ends_with("ICEs:\n"));
}

fn check_passes(clippy_path: &Path) {
    check(clippy_path, &PathBuf::from("../config/passes.toml"), None);
    let log_passes = fs::read_to_string("logs/passes_logs.txt").expect("couldn't read log file");
    assert!(!log_passes.contains("clippy::") && log_passes.ends_with("ICEs:\n"));
}

fn check_ci(clippy_path: &Path) {
    let file = create_temp_config("passes");
    check(clippy_path, file.path(), Some("ci_passes"));
    let log_passes = fs::read_to_string("logs/ci_passes_logs.txt").expect("couldn't read log file");
    assert!(!log_passes.contains("clippy::") && log_passes.ends_with("ICEs:\n"));

    let file = create_temp_config("integration");
    check(clippy_path, file.path(), Some("ci_integration"));
    let log_integration =
        fs::read_to_string("logs/ci_integration_logs.txt").expect("couldn't read log file");
    assert!(log_integration.ends_with("ICEs:\n"));
}

fn create_temp_config(name: &str) -> NamedTempFile {
    let mut tempfile = NamedTempFile::new().expect("failed to create tempfile");
    writeln!(tempfile, "[crates]").expect("couldn't write to tempfile");
    let diff = Command::new("git")
        .arg("diff")
        .args(&["origin/main", "--", &format!("config/{}.toml", name)])
        .stdout(Stdio::piped())
        .spawn()
        .expect("couldn't execute git diff");
    let grep = Command::new("grep")
        .args(&["-E", r"^\+\w+"])
        .stdin(diff.stdout.expect("failed to process git diff output"))
        .output()
        .expect("couldn't execute grep");
    let stdout = String::from_utf8_lossy(&grep.stdout);
    for l in stdout.lines().map(|l| &l[1..]) {
        writeln!(tempfile, "{}", l).expect("couldn't write to tempfile");
    }

    tempfile
}

fn main() {
    let opt: Opt = Opt::from_args();
    let clippy_path = PathBuf::from("rust-clippy").canonicalize().unwrap();
    match opt.mode {
        Mode::All => {
            check_integration(&clippy_path);
            check_passes(&clippy_path);
        }
        Mode::Passes => check_passes(&clippy_path),
        Mode::Integration => check_integration(&clippy_path),
        Mode::CI => check_ci(&clippy_path),
    }
}
