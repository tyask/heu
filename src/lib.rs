use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::process::Command;
use std::sync::mpsc;
use std::time::Instant;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub build: BuildConfig,
    pub test: TestConfig,
}

#[derive(Serialize, Deserialize)]
pub struct BuildConfig {
    pub enable: bool,
    pub command: String,
}

#[derive(Serialize, Deserialize)]
pub struct TestConfig {
    pub bin: String,
    pub cases: String,
    pub threads: usize,
    pub no_evaluate: bool,
    pub use_tester: bool,
    pub in_dir: String,
    pub out_dir: String,
    pub vis: String,
    pub tester: String,
    pub score_regex: String,
    pub comment_regex: String,
}

impl Config {
    pub fn default_config() -> Self {
        let cpus = num_cpus::get();
        Config {
            build: BuildConfig {
                enable: true,
                command: "cargo build --release --bin a --target-dir target -q".to_string(),
            },
            test: TestConfig {
                bin: "./target/release/a".to_string(),
                cases: "0-9".to_string(),
                threads: cpus,
                no_evaluate: false,
                use_tester: false,
                in_dir: "./tools/in".to_string(),
                out_dir: "./tools/out".to_string(),
                vis: "cargo run --manifest-path tools/Cargo.toml --bin vis --target-dir=tools/target -r".to_string(),
                tester: "cargo run --manifest-path tools/Cargo.toml --bin tester --target-dir=tools/target -r".to_string(),
                score_regex: "Score = (\\d+)".to_string(),
                comment_regex: "^# (.*)$".to_string(),
            },
        }
    }

    pub fn generate_toml_with_comments(&self) -> String {
        format!(
            r#"[build]
# ビルドを実行するか
enable = {}
# ビルドコマンド
command = "{}"

[test]
# 実行バイナリのパス
bin = "{}"
# テストケース範囲 (例: "0-9", "0 1 3-5")
cases = "{}"
# 並列スレッド数
threads = {}
# 評価なしで実行する (ビジュアライザによるスコア計算をスキップ)
no_evaluate = {}
# tester.exe を使用してインタラクティブ問題を実行する
use_tester = {}
# テスト入力ファイルのディレクトリパス
in_dir = "{}"
# テスト出力ファイルのディレクトリパス
out_dir = "{}"
# ビジュアライザ実行コマンド (引数として入力ファイルと出力ファイルが追加される)
vis = "{}"
# テスター実行コマンド (use_tester=true の場合に使用)
tester = "{}"
# ビジュアライザ出力からスコアを抽出する正規表現（第1キャプチャを数値として使用）
score_regex = "{}"
# stderr の各行からコメントを抽出する正規表現（第1キャプチャをコメント本文として使用）
comment_regex = "{}"
"#,
            self.build.enable,
            self.build.command,
            self.test.bin,
            self.test.cases,
            self.test.threads,
            self.test.no_evaluate,
            self.test.use_tester,
            self.test.in_dir,
            self.test.out_dir,
            self.test.vis,
            self.test.tester,
            self.test.score_regex,
            self.test.comment_regex,
        )
    }
}

/// ヒューリスティックコンテストのテストハーネス。
/// ソリューションのビルド・実行・ビジュアライザによるスコア評価を行う。
pub struct Heu {
    config: Config,
    cases: Vec<u32>,
    score_regex: Regex,
    comment_regex: Regex,
}

/// 1ケースの実行結果。
pub struct CaseResult {
    pub case: u32,
    pub inf: String,
    pub outf: String,
    pub visout: String,
    pub stderr: String,
    pub elapsed: f64,
    pub score: u64,
    comment_regex: Regex,
}

impl CaseResult {
    pub fn new(
        case: u32,
        inf: String,
        outf: String,
        visout: String,
        stderr: String,
        elapsed: f64,
        score_regex: &Regex,
        comment_regex: &Regex,
    ) -> Self {
        let score = Self::parse_score(&visout, score_regex);
        Self { case, inf, outf, visout, stderr, elapsed, score, comment_regex: comment_regex.clone() }
    }

    /// ビジュアライザ出力からスコアを抽出する。
    pub fn parse_score(visout: &str, score_regex: &Regex) -> u64 {
        for line in visout.lines() {
            if let Some(caps) = score_regex.captures(line) {
                if let Some(m) = caps.get(1) {
                    return m.as_str().parse().unwrap_or(0);
                }
            }
        }
        0
    }

    pub fn lookup_comments(&self) -> String {
        Self::lookup_comments_from(&self.stderr, &self.comment_regex)
    }

    /// stderrの各行からコメントを抽出し、"/" で結合する。
    pub fn lookup_comments_from(stderr: &str, comment_regex: &Regex) -> String {
        let mut cmts = String::new();
        for line in stderr.lines() {
            if let Some(caps) = comment_regex.captures(line) {
                let Some(rest) = caps.get(1).map(|m| m.as_str()) else {
                    continue;
                };
                if !cmts.is_empty() {
                    cmts.push('/');
                }
                cmts.push_str(rest);
            }
        }
        cmts
    }

    pub fn print(&self) {
        let cmts = self.lookup_comments();
        println!(
            "{:04} SCORE[{:>11}] ELAPSED[{:.2}s] CMTS[{}]",
            self.case,
            format_with_commas(self.score),
            self.elapsed,
            cmts
        );
    }

    /// 出力ファイルの内容をクリップボードにコピーする。
    pub fn clip(&self) {
        match fs::read_to_string(&self.outf) {
            Ok(content) => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(content);
                }
            }
            Err(_) => {}
        }
    }
}

/// 数値を3桁区切りカンマ付き文字列に変換する (例: 12345 -> "12,345")。
fn format_with_commas(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

impl Heu {
    pub fn new(config: Config) -> Self {
        let cases = parse_cases(
            &config.test.cases.split_whitespace().map(String::from).collect::<Vec<_>>(),
        );
        let score_regex = Regex::new(&config.test.score_regex).unwrap_or_else(|e| {
            panic!(
                "Invalid test.score_regex '{}': {}",
                config.test.score_regex, e
            )
        });
        let comment_regex = Regex::new(&config.test.comment_regex).unwrap_or_else(|e| {
            panic!(
                "Invalid test.comment_regex '{}': {}",
                config.test.comment_regex, e
            )
        });
        Self { config, cases, score_regex, comment_regex }
    }

    pub fn input_file(&self, case: u32) -> String {
        format!("{}/{:04}.txt", self.config.test.in_dir, case)
    }

    pub fn output_file(&self, case: u32) -> String {
        format!("{}/{:04}.txt", self.config.test.out_dir, case)
    }

    /// ビルドコマンドを実行する。enable が false の場合はスキップ。
    pub fn build(&self) -> io::Result<()> {
        if !self.config.build.enable {
            return Ok(());
        }
        let status = Self::command_from_str(&self.config.build.command)?.status()?;
        if !status.success() {
            return Err(io::Error::new(io::ErrorKind::Other, "build failed"));
        }
        Ok(())
    }

    /// ビルド後、全ケースを並列実行してスコアを表示する。
    pub fn execute(&self) -> io::Result<()> {
        self.build()?;

        if self.config.test.no_evaluate {
            return self.execute_run_only();
        }

        self.execute_multiprocess()
    }

    /// ビジュアライザによる評価なしでソリューションを実行する。
    fn execute_run_only(&self) -> io::Result<()> {
        for &case in &self.cases {
            let inf = self.input_file(case);
            let status = Self::command_from_str(&self.config.test.bin)?
                .env("INPUT_FILE", &inf)
                .env("IN_FILE", &inf)
                .stdin(std::process::Stdio::from(fs::File::open(&inf)?))
                .status()?;
            if !status.success() {
                return Err(io::Error::new(io::ErrorKind::Other, format!("case {} failed", case)));
            }
        }
        Ok(())
    }

    /// 1ケースを実行し、ビジュアライザで評価して結果を返す。
    fn execute_case(&self, case: u32) -> io::Result<CaseResult> {
        let inf = self.input_file(case);
        let outf = self.output_file(case);

        if let Some(parent) = std::path::Path::new(&outf).parent() {
            fs::create_dir_all(parent)?;
        }

        let input_data = fs::read(&inf)?;

        let start = Instant::now();
        let (stdout, stderr_bytes) = self.run_command(&inf, &input_data)?;
        let elapsed = start.elapsed().as_secs_f64();
        fs::write(&outf, &stdout)?;
        let stderr = String::from_utf8_lossy(&stderr_bytes).to_string();

        let visout = self.exe_vis(&inf, &outf)?;

        Ok(CaseResult::new(
            case,
            inf,
            outf,
            visout,
            stderr,
            elapsed,
            &self.score_regex,
            &self.comment_regex,
        ))
    }

    /// ソリューション(またはtester経由)を実行し、stdout/stderr(bytes) を返す。
    fn run_command(&self, inf: &str, input_data: &[u8]) -> io::Result<(Vec<u8>, Vec<u8>)> {
        let output = if self.config.test.use_tester {
            let mut cmd = Self::command_from_str(&self.config.test.tester)?;
            cmd.args(Self::parse_command_parts(&self.config.test.bin)?)
                .env("INPUT_FILE", inf)
                .env("IN_FILE", inf)
                .stdin(std::process::Stdio::from(fs::File::open(inf)?))
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            cmd.output()?
        } else {
            Self::command_from_str(&self.config.test.bin)?
                .env("INPUT_FILE", inf)
                .env("IN_FILE", inf)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    if let Some(stdin) = child.stdin.as_mut() {
                        stdin.write_all(input_data)?;
                    }
                    drop(child.stdin.take());
                    child.wait_with_output()
                })?
        };

        Ok((output.stdout, output.stderr))
    }

    fn parse_command_parts(cmd: &str) -> io::Result<Vec<String>> {
        let parts = shlex::split(cmd).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, format!("invalid command: {}", cmd))
        })?;
        if parts.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "empty command"));
        }
        Ok(parts)
    }

    /// コマンド文字列を分割して、シェルを介さず実行用 Command を作る。
    fn command_from_str(cmd: &str) -> io::Result<Command> {
        let mut parts = Self::parse_command_parts(cmd)?;
        let program = parts.remove(0);
        let mut command = Command::new(program);
        command.args(parts);
        Ok(command)
    }

    /// ビジュアライザコマンドを実行してスコア出力を返す。
    fn exe_vis(&self, inf: &str, outf: &str) -> io::Result<String> {
        let output = Self::command_from_str(&self.config.test.vis)?
            .arg(inf)
            .arg(outf)
            .output()?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// 全ケースを並列実行し、ケース番号昇順で結果を即時出力する。
    fn execute_multiprocess(&self) -> io::Result<()> {
        let n = self.cases.len();
        let (tx, rx) = mpsc::channel::<(usize, io::Result<CaseResult>)>();

        let run = |tx: mpsc::Sender<_>| {
            self.cases
                .par_iter()
                .enumerate()
                .for_each(|(i, &case)| {
                    let result = self.execute_case(case);
                    let _ = tx.send((i, result));
                });
        };

        let recv_result = std::thread::scope(|s| {
            let receiver = s.spawn(|| -> io::Result<()> {
                let mut buf: Vec<Option<CaseResult>> = (0..n).map(|_| None).collect();
                let mut next = 0;
                let mut total: u64 = 0;
                let mut last: Option<CaseResult> = None;

                for (i, result) in rx {
                    buf[i] = Some(result?);
                    while next < n {
                        if let Some(r) = buf[next].take() {
                            r.print();
                            total += r.score;
                            last = Some(r);
                            next += 1;
                        } else {
                            break;
                        }
                    }
                }

                if let Some(r) = last {
                    r.clip();
                }
                println!("TOTAL={}", format_with_commas(total));
                Ok(())
            });

            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(self.config.test.threads)
                .build()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            pool.install(|| run(tx));

            receiver.join().unwrap()
        });

        recv_result
    }
}

/// ケース指定文字列をパースする。"3-5" はレンジ、"3" は単一ケース。空なら 0-4。
pub fn parse_cases(args: &[String]) -> Vec<u32> {
    if args.is_empty() {
        return (0..5).collect();
    }

    let mut ret = Vec::new();
    for s in args {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() == 1 {
            if let Ok(n) = parts[0].parse::<u32>() {
                ret.push(n);
            }
        } else if parts.len() == 2 {
            if let (Ok(start), Ok(end)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                ret.extend(start..=end);
            }
        }
    }
    ret
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            build: BuildConfig {
                enable: false,
                command: String::new(),
            },
            test: TestConfig {
                bin: "./target/release/a".to_string(),
                cases: "0-9".to_string(),
                threads: 1,
                no_evaluate: false,
                use_tester: false,
                in_dir: "./tools/in".to_string(),
                out_dir: "./tools/out".to_string(),
                vis: String::new(),
                tester: String::new(),
                score_regex: "Score = (\\d+)".to_string(),
                comment_regex: "^# (.*)$".to_string(),
            },
        }
    }

    #[test]
    fn test_parse_cases_empty() {
        let args: Vec<String> = vec![];
        assert_eq!(parse_cases(&args), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_parse_cases_single() {
        let args = vec!["3".to_string()];
        assert_eq!(parse_cases(&args), vec![3]);
    }

    #[test]
    fn test_parse_cases_multiple() {
        let args = vec!["0".to_string(), "1".to_string(), "3".to_string()];
        assert_eq!(parse_cases(&args), vec![0, 1, 3]);
    }

    #[test]
    fn test_parse_cases_range() {
        let args = vec!["3-5".to_string()];
        assert_eq!(parse_cases(&args), vec![3, 4, 5]);
    }

    #[test]
    fn test_parse_cases_mixed() {
        let args = vec!["0".to_string(), "1".to_string(), "3-5".to_string()];
        assert_eq!(parse_cases(&args), vec![0, 1, 3, 4, 5]);
    }

    #[test]
    fn test_parse_score_normal() {
        let re = Regex::new(r"Score = (\d+)").unwrap();
        assert_eq!(CaseResult::parse_score("Score = 12345", &re), 12345);
    }

    #[test]
    fn test_parse_score_multiline() {
        let re = Regex::new(r"Score = (\d+)").unwrap();
        let visout = "some info\nScore = 67890\nother info";
        assert_eq!(CaseResult::parse_score(visout, &re), 67890);
    }

    #[test]
    fn test_parse_score_none() {
        let re = Regex::new(r"Score = (\d+)").unwrap();
        assert_eq!(CaseResult::parse_score("no score here", &re), 0);
    }

    #[test]
    fn test_parse_score_custom_regex() {
        let re = Regex::new(r"TotalScore: (\d+)").unwrap();
        assert_eq!(CaseResult::parse_score("TotalScore: 42", &re), 42);
    }

    #[test]
    fn test_lookup_comments_with_comments() {
        let re = Regex::new(r"^# (.*)$").unwrap();
        let cmts = CaseResult::lookup_comments_from("# foo\n# bar\n", &re);
        assert_eq!(cmts, "foo/bar");
    }

    #[test]
    fn test_lookup_comments_none() {
        let re = Regex::new(r"^# (.*)$").unwrap();
        let cmts = CaseResult::lookup_comments_from("no comments here\n", &re);
        assert_eq!(cmts, "");
    }

    #[test]
    fn test_lookup_comments_mixed() {
        let re = Regex::new(r"^# (.*)$").unwrap();
        let cmts = CaseResult::lookup_comments_from("debug line\n# comment1\nmore debug\n# comment2\n", &re);
        assert_eq!(cmts, "comment1/comment2");
    }

    #[test]
    fn test_lookup_comments_custom_regex() {
        let re = Regex::new(r"^\[cmt\] (.*)$").unwrap();
        let cmts = CaseResult::lookup_comments_from("[cmt] hello\n[cmt] world\n", &re);
        assert_eq!(cmts, "hello/world");
    }

    #[test]
    #[should_panic(expected = "Invalid test.score_regex")]
    fn test_heu_new_invalid_score_regex_panics() {
        let mut cfg = test_config();
        cfg.test.score_regex = "(".to_string();
        let _ = Heu::new(cfg);
    }

    #[test]
    #[should_panic(expected = "Invalid test.comment_regex")]
    fn test_heu_new_invalid_comment_regex_panics() {
        let mut cfg = test_config();
        cfg.test.comment_regex = "(".to_string();
        let _ = Heu::new(cfg);
    }

    #[test]
    fn test_input_file() {
        let heu = Heu::new(test_config());
        assert_eq!(heu.input_file(3), "./tools/in/0003.txt");
    }

    #[test]
    fn test_output_file() {
        let heu = Heu::new(test_config());
        assert_eq!(heu.output_file(3), "./tools/out/0003.txt");
    }
}
