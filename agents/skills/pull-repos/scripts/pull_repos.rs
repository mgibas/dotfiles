#!/usr/bin/env rust-script
//! ```cargo
//! [package]
//! edition = "2021"
//!
//! [dependencies]
//! clap = { version = "=4.6.7", default-features = false, features = ["std", "derive", "help", "usage", "error-context", "suggestions"] }
//! libc = "=0.2.190"
//! ```

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};
use std::collections::HashMap;
use std::env;
use std::ffi::{CStr, CString, OsStr, OsString};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitCode, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const PROGRAM: &str = "pull_repos.rs";
const REMOTE: &str = "origin";
const LOCK_FILE_NAME: &str = "pull-repos.lock";
const IGNORED_DIRECTORY_NAMES: &[&str] = &["node_modules"];
const OPERATION_MARKERS: &[&str] = &[
    "MERGE_HEAD",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "BISECT_LOG",
    "rebase-merge",
    "rebase-apply",
    "sequencer",
    "index.lock",
    "shallow.lock",
];
const GIT_BASE_ARGS: &[&str] = &["-c", "gc.auto=0", "-c", "maintenance.auto=false"];
const TERMINATE_GRACE: Duration = Duration::from_secs(2);
const RESULT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const DETAIL_LIMIT: usize = 200;
const DEFAULT_ROOT: &str = "~/Projects";
const DEFAULT_JOBS: usize = 4;
const DEFAULT_TIMEOUT_SECONDS: f64 = 120.0;
const DEFAULT_MAX_DEPTH: usize = 2;
const INTERRUPTED_MESSAGE: &str = "\ninterrupted; in-flight repositories finished, remaining skipped\n";

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Updated,
    Current,
    WouldPull,
    Skipped,
    Failed,
}

impl Outcome {
    const ALL: [Outcome; 5] = [
        Outcome::Updated,
        Outcome::Current,
        Outcome::WouldPull,
        Outcome::Skipped,
        Outcome::Failed,
    ];

    fn label(self) -> &'static str {
        match self {
            Outcome::Updated => "updated",
            Outcome::Current => "current",
            Outcome::WouldPull => "would-pull",
            Outcome::Skipped => "skipped",
            Outcome::Failed => "failed",
        }
    }
}

#[derive(Debug)]
enum Stop {
    Skip(String),
    Failure(String),
    Timeout(String),
    Unexpected(String),
}

struct RepoResult {
    name: PathBuf,
    outcome: Outcome,
    seconds: f64,
    detail: String,
}

#[derive(Default)]
struct Inventory {
    checkouts: Vec<PathBuf>,
    linked_worktrees: Vec<PathBuf>,
    unreadable: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PhaseLimit {
    seconds: f64,
    duration: Option<Duration>,
}

impl PhaseLimit {
    fn from_seconds(seconds: f64) -> PhaseLimit {
        PhaseLimit {
            seconds,
            duration: Duration::try_from_secs_f64(seconds).ok(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Options {
    root: PathBuf,
    jobs: usize,
    timeout: PhaseLimit,
    max_depth: usize,
    dry_run: bool,
}

fn positive_int(value: &str) -> Result<usize, String> {
    let number: i128 = value.parse().map_err(|_| format!("'{value}' is not a whole number"))?;
    if number < 1 {
        return Err("must be at least 1".to_string());
    }
    Ok(usize::try_from(number).unwrap_or(usize::MAX))
}

fn python_number_text(text: &str) -> Option<String> {
    let text = python_strip(text);
    let characters: Vec<char> = text.chars().collect();
    for (index, character) in characters.iter().enumerate() {
        if *character == '_' {
            let digit_before = index > 0 && characters[index - 1].is_ascii_digit();
            let digit_after = characters.get(index + 1).is_some_and(char::is_ascii_digit);
            if !digit_before || !digit_after {
                return None;
            }
        }
    }
    Some(text.replace('_', ""))
}

fn positive_finite_seconds(value: &str) -> Result<f64, String> {
    let number: f64 = python_number_text(value)
        .and_then(|text| text.parse().ok())
        .ok_or_else(|| format!("'{value}' is not a number"))?;
    if !number.is_finite() || number <= 0.0 {
        return Err("must be a finite number greater than 0".to_string());
    }
    Ok(number)
}

#[derive(Debug, Parser)]
#[command(
    name = PROGRAM,
    bin_name = PROGRAM,
    about = "Fast-forward pull clean default-branch Git checkouts under a root directory.",
    args_override_self = true,
    after_help = "Exit status:\n  0    no checkout failed\n  1    a checkout failed, or the run did not start (root, lock, or setup error)\n  2    usage error\n  130  interrupted"
)]
struct Cli {
    #[arg(long, default_value = DEFAULT_ROOT, help = "directory to search for Git checkouts")]
    root: PathBuf,
    #[arg(long, default_value_t = DEFAULT_JOBS, value_parser = positive_int, allow_negative_numbers = true, help = "checkouts to process in parallel")]
    jobs: usize,
    #[arg(
        long,
        default_value_t = DEFAULT_TIMEOUT_SECONDS,
        value_parser = positive_finite_seconds,
        allow_negative_numbers = true,
        help = "seconds per phase per repository; phases are checks plus fetch, then checks plus merge"
    )]
    timeout: f64,
    #[arg(long, default_value_t = DEFAULT_MAX_DEPTH, value_parser = positive_int, allow_negative_numbers = true, help = "directory levels below root to search")]
    max_depth: usize,
    #[arg(long, help = "run local checks only; no fetch or merge")]
    dry_run: bool,
}

fn is_negative_number(value: &OsStr) -> bool {
    let Some(number) = value.as_bytes().strip_prefix(b"-") else {
        return false;
    };
    match number.iter().position(|byte| *byte == b'.') {
        None => !number.is_empty() && number.iter().all(u8::is_ascii_digit),
        Some(dot) => number[..dot].iter().all(u8::is_ascii_digit) && number.len() > dot + 1 && number[dot + 1..].iter().all(u8::is_ascii_digit),
    }
}

fn join_negative_number_root_values(arguments: &[OsString]) -> Vec<OsString> {
    let mut joined = Vec::with_capacity(arguments.len());
    let mut index = 0;
    while index < arguments.len() {
        match arguments.get(index + 1) {
            Some(value) if index > 0 && arguments[index] == "--root" && is_negative_number(value) => {
                let mut argument = OsString::from("--root=");
                argument.push(value);
                joined.push(argument);
                index += 2;
            }
            _ => {
                joined.push(arguments[index].clone());
                index += 1;
            }
        }
    }
    joined
}

fn parse_cli<I, T>(arguments: I) -> Result<Cli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let arguments: Vec<OsString> = arguments.into_iter().map(Into::into).collect();
    let cli = Cli::try_parse_from(join_negative_number_root_values(&arguments))?;
    if arguments.iter().skip(1).any(|argument| argument == "--") {
        return Err(Cli::command().error(ErrorKind::UnknownArgument, "unexpected argument '--' found"));
    }
    Ok(cli)
}

impl From<Cli> for Options {
    fn from(cli: Cli) -> Options {
        Options {
            root: cli.root,
            jobs: cli.jobs,
            timeout: PhaseLimit::from_seconds(cli.timeout),
            max_depth: cli.max_depth,
            dry_run: cli.dry_run,
        }
    }
}

enum GitMarker {
    Directory,
    File,
    Other,
}

fn git_marker(path: &Path) -> io::Result<Option<GitMarker>> {
    match fs::symlink_metadata(path.join(".git")) {
        Ok(metadata) if metadata.is_dir() => Ok(Some(GitMarker::Directory)),
        Ok(metadata) if metadata.is_file() => Ok(Some(GitMarker::File)),
        Ok(_) => Ok(Some(GitMarker::Other)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn is_ignored_name(name: &OsStr) -> bool {
    let bytes = name.as_bytes();
    bytes.starts_with(b".") || IGNORED_DIRECTORY_NAMES.iter().any(|ignored| bytes == ignored.as_bytes())
}

fn discover(root: &Path, max_depth: usize) -> Inventory {
    let mut inventory = Inventory::default();
    walk(root, 1, max_depth, &mut inventory);
    inventory
}

fn walk(directory: &Path, depth: usize, max_depth: usize, inventory: &mut Inventory) {
    let mut children = match fs::read_dir(directory).and_then(|entries| entries.collect::<io::Result<Vec<_>>>()) {
        Ok(children) => children,
        Err(_) => {
            inventory.unreadable.push(directory.to_path_buf());
            return;
        }
    };
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        if is_ignored_name(&child.file_name()) {
            continue;
        }
        if !child.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let path = child.path();
        match git_marker(&path) {
            Err(_) => inventory.unreadable.push(path),
            Ok(Some(GitMarker::Directory)) => inventory.checkouts.push(path),
            Ok(Some(GitMarker::File)) => inventory.linked_worktrees.push(path),
            Ok(Some(GitMarker::Other)) => {}
            Ok(None) if depth < max_depth => walk(&path, depth + 1, max_depth, inventory),
            Ok(None) => {}
        }
    }
}

fn is_python_whitespace(character: char) -> bool {
    character.is_whitespace() || ('\x1c'..='\x1f').contains(&character)
}

fn python_strip(text: &str) -> &str {
    text.trim_matches(is_python_whitespace)
}

fn python_split_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut characters = text.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        let is_break = matches!(
            character,
            '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}'
        );
        if !is_break {
            continue;
        }
        lines.push(&text[start..index]);
        start = index + character.len_utf8();
        if character == '\r' && matches!(characters.peek(), Some((_, '\n'))) {
            characters.next();
            start += 1;
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

fn redact(text: &str) -> String {
    let characters: Vec<char> = text.chars().collect();
    let mut redacted = String::with_capacity(text.len());
    let mut index = 0;
    while index < characters.len() {
        if characters[index..].starts_with(&[':', '/', '/']) {
            let credentials_start = index + 3;
            let mut end = credentials_start;
            while end < characters.len()
                && characters[end] != '/'
                && characters[end] != '@'
                && !is_python_whitespace(characters[end])
            {
                end += 1;
            }
            if end > credentials_start && characters.get(end) == Some(&'@') {
                redacted.push_str("://***@");
                index = end + 1;
                continue;
            }
        }
        redacted.push(characters[index]);
        index += 1;
    }
    redacted
}

fn truncate(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

fn error_summary(text: &str) -> String {
    let lines: Vec<&str> = python_split_lines(text)
        .into_iter()
        .map(python_strip)
        .filter(|line| !line.is_empty())
        .collect();
    let chosen = lines
        .iter()
        .find(|line| line.starts_with("fatal:") || line.starts_with("error:"))
        .or(lines.first());
    chosen.map(|line| truncate(&redact(line), DETAIL_LIMIT)).unwrap_or_default()
}

fn format_g(value: f64) -> String {
    if value == 0.0 || !value.is_finite() {
        return format!("{value}");
    }
    let scientific = format!("{value:.5e}");
    let (mantissa, exponent) = scientific.split_once('e').expect("scientific format has an exponent");
    let exponent: i32 = exponent.parse().expect("scientific exponent is an integer");
    if (-4..6).contains(&exponent) {
        let decimals = (5 - exponent) as usize;
        trim_fraction(&format!("{value:.decimals$}"))
    } else {
        let sign = if exponent < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", trim_fraction(mantissa), exponent.abs())
    }
}

fn trim_fraction(number: &str) -> String {
    if number.contains('.') {
        number.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        number.to_string()
    }
}

fn signal_group(pid: libc::pid_t, signal: libc::c_int) {
    unsafe {
        libc::killpg(pid, signal);
    }
}

enum ChildEvent {
    Exited,
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
}

#[derive(Default)]
struct ChildProgress {
    exited: bool,
    stdout: Option<Vec<u8>>,
    stderr: Option<Vec<u8>>,
}

impl ChildProgress {
    fn apply(&mut self, event: ChildEvent) {
        match event {
            ChildEvent::Exited => self.exited = true,
            ChildEvent::Stdout(bytes) => self.stdout = Some(bytes),
            ChildEvent::Stderr(bytes) => self.stderr = Some(bytes),
        }
    }

    fn is_complete(&self) -> bool {
        self.exited && self.stdout.is_some() && self.stderr.is_some()
    }
}

fn spawn_reader<R: Read + Send + 'static>(mut source: R, events: mpsc::Sender<ChildEvent>, wrap: fn(Vec<u8>) -> ChildEvent) {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = source.read_to_end(&mut bytes);
        let _ = events.send(wrap(bytes));
    });
}

fn spawn_exit_watcher(pid: libc::pid_t, events: mpsc::Sender<ChildEvent>) {
    thread::spawn(move || {
        loop {
            let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            let status = unsafe { libc::waitid(libc::P_PID, pid as libc::id_t, &mut info, libc::WEXITED | libc::WNOWAIT) };
            if status == 0 || io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
                break;
            }
        }
        let _ = events.send(ChildEvent::Exited);
    });
}

fn receive_until(events: &mpsc::Receiver<ChildEvent>, progress: &mut ChildProgress, deadline: Option<Instant>) -> bool {
    while !progress.is_complete() {
        let event = match deadline {
            None => events.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected),
            Some(deadline) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return false;
                }
                events.recv_timeout(remaining)
            }
        };
        match event {
            Ok(event) => progress.apply(event),
            Err(mpsc::RecvTimeoutError::Timeout) => return false,
            Err(mpsc::RecvTimeoutError::Disconnected) => return progress.exited,
        }
    }
    true
}

enum Completion {
    Finished { status: ExitStatus, stdout: Vec<u8>, stderr: Vec<u8> },
    TimedOut,
}

fn wait_with_deadline(mut child: Child, deadline: Option<Instant>) -> io::Result<Completion> {
    let pid = child.id() as libc::pid_t;
    let (sender, events) = mpsc::channel();
    spawn_reader(child.stdout.take().expect("stdout is piped"), sender.clone(), ChildEvent::Stdout);
    spawn_reader(child.stderr.take().expect("stderr is piped"), sender.clone(), ChildEvent::Stderr);
    spawn_exit_watcher(pid, sender);
    let mut progress = ChildProgress::default();
    if receive_until(&events, &mut progress, deadline) {
        let status = child.wait()?;
        return Ok(Completion::Finished {
            status,
            stdout: progress.stdout.unwrap_or_default(),
            stderr: progress.stderr.unwrap_or_default(),
        });
    }
    signal_group(pid, libc::SIGTERM);
    if !receive_until(&events, &mut progress, Some(Instant::now() + TERMINATE_GRACE)) {
        signal_group(pid, libc::SIGKILL);
        while !progress.exited {
            match events.recv() {
                Ok(event) => progress.apply(event),
                Err(_) => break,
            }
        }
    }
    child.wait()?;
    Ok(Completion::TimedOut)
}

fn exit_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or_else(|| -status.signal().unwrap_or(0))
}

fn git_command(repo: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .args(GIT_BASE_ARGS)
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if env::var_os("GIT_SSH_COMMAND").is_none() {
        command.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
    }
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command
}

struct GitOutput {
    code: i32,
    stdout: String,
    stderr: String,
}

struct RepoGit<'a> {
    repo: &'a Path,
    limit: PhaseLimit,
    deadline: Option<Instant>,
}

impl<'a> RepoGit<'a> {
    fn new(repo: &'a Path, limit: PhaseLimit) -> RepoGit<'a> {
        let mut git = RepoGit { repo, limit, deadline: None };
        git.start_phase();
        git
    }

    fn start_phase(&mut self) {
        self.deadline = self.limit.duration.and_then(|duration| Instant::now().checked_add(duration));
    }

    fn run(&self, args: &[&str]) -> Result<GitOutput, Stop> {
        let limit = format_g(self.limit.seconds);
        if let Some(deadline) = self.deadline {
            if deadline <= Instant::now() {
                return Err(Stop::Timeout(format!("git {} not started, phase limit {limit}s reached", args[0])));
            }
        }
        let child = git_command(self.repo, args)
            .spawn()
            .map_err(|error| Stop::Unexpected(format!("unexpected error starting git: {error}")))?;
        match wait_with_deadline(child, self.deadline) {
            Ok(Completion::Finished { status, stdout, stderr }) => Ok(GitOutput {
                code: exit_code(status),
                stdout: python_strip(&String::from_utf8_lossy(&stdout)).to_string(),
                stderr: String::from_utf8_lossy(&stderr).into_owned(),
            }),
            Ok(Completion::TimedOut) => Err(Stop::Timeout(format!("git {} timed out, phase limit {limit}s", args[0]))),
            Err(error) => Err(Stop::Unexpected(format!("unexpected error waiting for git: {error}"))),
        }
    }

    fn query(&self, args: &[&str]) -> Result<Option<String>, Stop> {
        let output = self.run(args)?;
        match output.code {
            0 => Ok(Some(output.stdout)),
            1 => Ok(None),
            _ => Err(Stop::Failure(format!("git {} failed: {}", args[0], error_summary(&output.stderr)))),
        }
    }

    fn execute(&self, args: &[&str]) -> Result<String, Stop> {
        let output = self.run(args)?;
        if output.code != 0 {
            return Err(Stop::Failure(format!("git {} failed: {}", args[0], error_summary(&output.stderr))));
        }
        Ok(output.stdout)
    }
}

fn ensure_no_operation_in_progress(repo: &Path) -> Result<(), Stop> {
    for marker in OPERATION_MARKERS {
        if fs::symlink_metadata(repo.join(".git").join(marker)).is_ok() {
            return Err(Stop::Skip(format!("git operation in progress ({marker})")));
        }
    }
    Ok(())
}

fn check_eligible(git: &RepoGit) -> Result<(String, String), Stop> {
    ensure_no_operation_in_progress(git.repo)?;

    let branch = git
        .query(&["symbolic-ref", "-q", "--short", "HEAD"])?
        .ok_or_else(|| Stop::Skip("detached HEAD".to_string()))?;

    if git.query(&["config", "--get", &format!("remote.{REMOTE}.url")])?.is_none() {
        return Err(Stop::Skip(format!("remote {REMOTE} not configured")));
    }

    let remote_prefix = format!("{REMOTE}/");
    let default = git
        .query(&["symbolic-ref", "-q", "--short", &format!("refs/remotes/{REMOTE}/HEAD")])?
        .and_then(|remote_head| remote_head.strip_prefix(&remote_prefix).map(str::to_string))
        .ok_or_else(|| Stop::Skip(format!("default branch unknown ({REMOTE}/HEAD not set)")))?;

    if branch != default {
        return Err(Stop::Skip(format!("on branch {branch}, default branch is {default}")));
    }

    let upstream_remote = git.query(&["config", "--get", &format!("branch.{branch}.remote")])?;
    let upstream_merge = git.query(&["config", "--get", &format!("branch.{branch}.merge")])?;
    if upstream_remote.as_deref() != Some(REMOTE) || upstream_merge != Some(format!("refs/heads/{default}")) {
        return Err(Stop::Skip(format!("upstream of {branch} is not {REMOTE}/{default}")));
    }

    let changes = git.execute(&["status", "--porcelain", "--untracked-files=normal"])?;
    if !changes.is_empty() {
        let entries = python_split_lines(&changes).len();
        return Err(Stop::Skip(format!("working tree not clean ({entries} entries)")));
    }
    Ok((branch, default))
}

fn merge_target(git: &RepoGit, target: &str) -> Result<(), Stop> {
    match git.execute(&["merge", "--ff-only", "--no-overwrite-ignore", "--no-autostash", "--quiet", target]) {
        Err(Stop::Timeout(timeout)) => {
            let lock = if fs::symlink_metadata(git.repo.join(".git").join("index.lock")).is_ok() {
                " index.lock remains."
            } else {
                ""
            };
            Err(Stop::Failure(format!("{timeout}; checkout may be partly updated, run git status.{lock}")))
        }
        other => other.map(|_| ()),
    }
}

fn short(sha: &str) -> String {
    sha.chars().take(8).collect()
}

fn pull_checkout(git: &mut RepoGit, dry_run: bool) -> Result<(Outcome, String), Stop> {
    let (branch, default) = check_eligible(git)?;
    if dry_run {
        return Ok((Outcome::WouldPull, format!("{branch} from {REMOTE}/{default}")));
    }

    let tracking_ref = format!("refs/remotes/{REMOTE}/{default}");
    git.execute(&[
        "fetch",
        "--quiet",
        "--no-tags",
        "--no-recurse-submodules",
        REMOTE,
        &format!("+refs/heads/{default}:{tracking_ref}"),
    ])?;

    git.start_phase();
    if check_eligible(git)? != (branch.clone(), default.clone()) {
        return Err(Stop::Skip("branch changed during fetch".to_string()));
    }
    let head = git.execute(&["rev-parse", "--verify", "HEAD^{commit}"])?;
    let target = git.execute(&["rev-parse", "--verify", &format!("{tracking_ref}^{{commit}}")])?;
    if head == target {
        return Ok((Outcome::Current, format!("{branch} at {}", short(&head))));
    }
    if git.query(&["merge-base", "--is-ancestor", &target, &head])?.is_some() {
        return Err(Stop::Skip(format!("{branch} has local commits not in {REMOTE}/{default}")));
    }
    if git.query(&["merge-base", "--is-ancestor", &head, &target])?.is_none() {
        return Err(Stop::Skip(format!("{branch} diverged from {REMOTE}/{default}")));
    }
    let count = git.execute(&["rev-list", "--count", &format!("{head}..{target}")])?;
    merge_target(git, &target)?;
    Ok((Outcome::Updated, format!("{}..{} (+{count} commits)", short(&head), short(&target))))
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| message.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_string())
}

fn process_checkout(repo: &Path, root: &Path, dry_run: bool, limit: PhaseLimit) -> RepoResult {
    let started = Instant::now();
    let name = repo.strip_prefix(root).unwrap_or(repo).to_path_buf();
    let attempt = panic::catch_unwind(AssertUnwindSafe(|| pull_checkout(&mut RepoGit::new(repo, limit), dry_run)));
    let (outcome, detail) = match attempt {
        Ok(Ok(pulled)) => pulled,
        Ok(Err(Stop::Skip(detail))) => (Outcome::Skipped, detail),
        Ok(Err(Stop::Failure(detail) | Stop::Timeout(detail))) => (Outcome::Failed, redact(&detail)),
        Ok(Err(Stop::Unexpected(detail))) => (Outcome::Failed, truncate(&redact(&detail), DETAIL_LIMIT)),
        Err(payload) => {
            let detail = format!("unexpected panic: {}", panic_message(payload.as_ref()));
            (Outcome::Failed, truncate(&redact(&detail), DETAIL_LIMIT))
        }
    };
    RepoResult {
        name,
        outcome,
        seconds: started.elapsed().as_secs_f64(),
        detail,
    }
}

fn emit(output: &mut dyn Write, bytes: &[u8]) {
    let _ = output.write_all(bytes);
    let _ = output.flush();
}

fn write_result(output: &mut dyn Write, result: &RepoResult) {
    let mut line = format!("{:<10} {:6.2}s  ", result.outcome.label(), result.seconds).into_bytes();
    line.extend_from_slice(result.name.as_os_str().as_bytes());
    line.extend_from_slice(b"  ");
    line.extend_from_slice(result.detail.as_bytes());
    line.push(b'\n');
    emit(output, &line);
}

fn write_summary(output: &mut dyn Write, results: &[RepoResult], inventory: &Inventory, total_seconds: f64, jobs: usize) {
    let shown: Vec<String> = Outcome::ALL
        .iter()
        .map(|outcome| (outcome, results.iter().filter(|result| result.outcome == *outcome).count()))
        .filter(|(_, count)| *count > 0)
        .map(|(outcome, count)| format!("{} {count}", outcome.label()))
        .collect();
    let repo_seconds: f64 = results.iter().map(|result| result.seconds).sum();
    let unreadable = if inventory.unreadable.is_empty() {
        String::new()
    } else {
        format!("  unreadable-directories {}", inventory.unreadable.len())
    };
    let summary = format!(
        "\ncheckouts {}  {}  linked-worktrees-ignored {}{unreadable}\ntotal {total_seconds:.2}s  sum-of-repo-time {repo_seconds:.2}s  jobs {jobs}\n",
        results.len(),
        shown.join("  "),
        inventory.linked_worktrees.len(),
    );
    emit(output, summary.as_bytes());
}

fn home_directory(user: Option<&OsStr>) -> Option<PathBuf> {
    if user.is_none() {
        if let Some(home) = env::var_os("HOME") {
            return Some(PathBuf::from(home));
        }
    }
    let entry = unsafe {
        match user {
            Some(user) => {
                let name = CString::new(user.as_bytes()).ok()?;
                libc::getpwnam(name.as_ptr())
            }
            None => libc::getpwuid(libc::getuid()),
        }
    };
    if entry.is_null() {
        return None;
    }
    let directory = unsafe { CStr::from_ptr((*entry).pw_dir) };
    Some(PathBuf::from(OsStr::from_bytes(directory.to_bytes())))
}

fn expand_user(path: &Path) -> PathBuf {
    let bytes = path.as_os_str().as_bytes();
    if !bytes.starts_with(b"~") {
        return path.to_path_buf();
    }
    let separator = bytes.iter().position(|byte| *byte == b'/').unwrap_or(bytes.len());
    let user = &bytes[1..separator];
    let home = home_directory(if user.is_empty() { None } else { Some(OsStr::from_bytes(user)) });
    match home {
        Some(home) => {
            let rest = &bytes[separator..];
            let mut expanded = home.into_os_string().into_vec();
            while expanded.len() > 1 && expanded.ends_with(b"/") {
                expanded.pop();
            }
            expanded.extend_from_slice(rest);
            PathBuf::from(OsString::from_vec(expanded))
        }
        None => path.to_path_buf(),
    }
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                if !normalized.pop() || normalized.as_os_str().is_empty() {
                    normalized.push("/");
                }
            }
            Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn absolute(path: &Path) -> PathBuf {
    let path = if path.as_os_str().is_empty() { Path::new(".") } else { path };
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir().unwrap_or_else(|_| PathBuf::from("/")).join(path)
    };
    normalize_lexically(&joined)
}

enum PendingPart {
    Name(Vec<u8>),
    ResolvedLink(Vec<u8>),
}

fn path_bytes(path: &[u8]) -> PathBuf {
    PathBuf::from(OsString::from_vec(path.to_vec()))
}

fn resolve_root(path: &Path) -> io::Result<PathBuf> {
    let expanded = expand_user(path).into_os_string().into_vec();
    let filename = if expanded.is_empty() { b".".to_vec() } else { expanded };
    let mut resolved = if filename.starts_with(b"/") {
        b"/".to_vec()
    } else {
        env::current_dir()?.into_os_string().into_vec()
    };
    let trimmed_length = filename.iter().rposition(|byte| *byte != b'/').map_or(0, |index| index + 1);
    let mut pending: Vec<PendingPart> =
        filename[..trimmed_length].split(|byte| *byte == b'/').rev().map(|name| PendingPart::Name(name.to_vec())).collect();
    let mut links: HashMap<Vec<u8>, Option<Vec<u8>>> = HashMap::new();

    while let Some(part) = pending.pop() {
        let name = match part {
            PendingPart::ResolvedLink(link) => {
                links.insert(link, Some(resolved.clone()));
                continue;
            }
            PendingPart::Name(name) => name,
        };
        if name.is_empty() || name == b"." {
            continue;
        }
        if name == b".." {
            let separator = resolved.iter().rposition(|byte| *byte == b'/').unwrap_or(0);
            resolved.truncate(separator.max(1));
            continue;
        }
        let mut candidate = resolved.clone();
        if candidate != b"/" {
            candidate.push(b'/');
        }
        candidate.extend_from_slice(&name);
        let is_link = fs::symlink_metadata(path_bytes(&candidate)).is_ok_and(|metadata| metadata.file_type().is_symlink());
        if !is_link {
            resolved = candidate;
            continue;
        }
        match links.get(&candidate) {
            Some(Some(target)) => {
                resolved = target.clone();
                continue;
            }
            Some(None) => {
                resolved = candidate;
                continue;
            }
            None => {}
        }
        let Ok(target) = fs::read_link(path_bytes(&candidate)) else {
            resolved = candidate;
            continue;
        };
        let target = target.into_os_string().into_vec();
        if target.starts_with(b"/") {
            resolved = b"/".to_vec();
        }
        links.insert(candidate.clone(), None);
        pending.push(PendingPart::ResolvedLink(candidate));
        pending.extend(target.split(|byte| *byte == b'/').rev().map(|name| PendingPart::Name(name.to_vec())));
    }
    Ok(path_bytes(&resolved))
}

fn is_usable_temp_directory(directory: &Path) -> bool {
    for attempt in 0..100 {
        let probe = directory.join(format!(".pull-repos-probe-{}-{attempt}", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&probe) {
            Ok(mut file) => {
                let written = file.write_all(b"blat").is_ok();
                drop(file);
                let _ = fs::remove_file(&probe);
                return written;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(_) => return false,
        }
    }
    false
}

fn python_temp_directory() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = ["TMPDIR", "TEMP", "TMP"]
        .iter()
        .filter_map(env::var_os)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .collect();
    candidates.extend(["/tmp", "/var/tmp", "/usr/tmp"].map(PathBuf::from));
    if let Ok(current) = env::current_dir() {
        candidates.push(current);
    }
    candidates.into_iter().map(|candidate| absolute(&candidate)).find(|candidate| is_usable_temp_directory(candidate))
}

fn acquire_lock(path: &Path) -> Result<File, String> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|error| format!("cannot open lock file {}: {error}", path.display()))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(format!("another pull_repos run is active (lock: {})", path.display())),
        Err(TryLockError::Error(error)) => Err(format!("cannot lock {}: {error}", path.display())),
    }
}

fn run_checkouts(
    checkouts: &[PathBuf],
    root: &Path,
    options: &Options,
    interrupted: &AtomicBool,
    output: &mut dyn Write,
) -> Option<Vec<RepoResult>> {
    let next = AtomicUsize::new(0);
    let (sender, receiver) = mpsc::channel();
    let mut results = Vec::new();
    let mut stopped = false;
    thread::scope(|scope| {
        for _ in 0..options.jobs.min(checkouts.len()) {
            let sender = sender.clone();
            let next = &next;
            scope.spawn(move || loop {
                if interrupted.load(Ordering::SeqCst) {
                    break;
                }
                let index = next.fetch_add(1, Ordering::SeqCst);
                let Some(repo) = checkouts.get(index) else { break };
                let result = process_checkout(repo, root, options.dry_run, options.timeout);
                if sender.send(result).is_err() {
                    break;
                }
            });
        }
        drop(sender);
        loop {
            if interrupted.load(Ordering::SeqCst) {
                stopped = true;
                break;
            }
            match receiver.recv_timeout(RESULT_POLL_INTERVAL) {
                Ok(result) => {
                    if interrupted.load(Ordering::SeqCst) {
                        stopped = true;
                        break;
                    }
                    write_result(output, &result);
                    results.push(result);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
    if stopped {
        None
    } else {
        Some(results)
    }
}

fn run(options: &Options, lock_path: &Path, interrupted: &AtomicBool, output: &mut dyn Write, errors: &mut dyn Write) -> u8 {
    let root = match resolve_root(&options.root) {
        Ok(root) => root,
        Err(error) => {
            emit(errors, format!("cannot resolve root {}: {error}\n", options.root.display()).as_bytes());
            return 1;
        }
    };
    if !root.is_dir() {
        emit(errors, format!("root is not a directory: {}\n", root.display()).as_bytes());
        return 1;
    }

    let _lock = if options.dry_run {
        None
    } else {
        match acquire_lock(lock_path) {
            Ok(file) => Some(file),
            Err(message) => {
                emit(errors, format!("{message}\n").as_bytes());
                return 1;
            }
        }
    };
    let inventory = discover(&root, options.max_depth);
    if interrupted.load(Ordering::SeqCst) {
        emit(errors, INTERRUPTED_MESSAGE.as_bytes());
        return 130;
    }

    let mode = if options.dry_run { "dry-run" } else { "live" };
    let mut header = b"root ".to_vec();
    header.extend_from_slice(root.as_os_str().as_bytes());
    header.extend_from_slice(
        format!(
            "  mode {mode}  jobs {}  phase-timeout {}s  checkouts {}\n\n",
            options.jobs,
            format_g(options.timeout.seconds),
            inventory.checkouts.len()
        )
        .as_bytes(),
    );
    emit(output, &header);

    let started = Instant::now();
    let Some(results) = run_checkouts(&inventory.checkouts, &root, options, interrupted, output) else {
        emit(errors, INTERRUPTED_MESSAGE.as_bytes());
        return 130;
    };
    write_summary(output, &results, &inventory, started.elapsed().as_secs_f64(), options.jobs);
    if results.iter().any(|result| result.outcome == Outcome::Failed) {
        1
    } else {
        0
    }
}

extern "C" fn on_interrupt(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

fn install_interrupt_handler() -> io::Result<()> {
    unsafe {
        let mut previous: libc::sigaction = std::mem::zeroed();
        if libc::sigaction(libc::SIGINT, std::ptr::null(), &mut previous) != 0 {
            return Err(io::Error::last_os_error());
        }
        if previous.sa_sigaction == libc::SIG_IGN {
            return Ok(());
        }
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = on_interrupt as extern "C" fn(libc::c_int) as libc::sighandler_t;
        action.sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&mut action.sa_mask);
        if libc::sigaction(libc::SIGINT, &action, std::ptr::null_mut()) != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let options = match parse_cli(env::args_os()) {
        Ok(cli) => Options::from(cli),
        Err(error) => {
            let _ = error.print();
            return ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(2));
        }
    };
    if let Err(error) = install_interrupt_handler() {
        eprintln!("cannot install interrupt handler: {error}");
        return ExitCode::from(1);
    }
    let lock_path = if options.dry_run {
        PathBuf::new()
    } else {
        match python_temp_directory() {
            Some(directory) => directory.join(LOCK_FILE_NAME),
            None => {
                eprintln!("no usable temporary directory found for the lock file");
                return ExitCode::from(1);
            }
        }
    };
    let code = run(&options, &lock_path, &INTERRUPTED, &mut io::stdout().lock(), &mut io::stderr().lock());
    ExitCode::from(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::sync::atomic::AtomicU64;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    fn require_isolated_git_config() {
        assert_eq!(
            env::var_os("GIT_CONFIG_GLOBAL").as_deref(),
            Some(OsStr::new("/dev/null")),
            "run tests with GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1"
        );
        assert!(env::var_os("GIT_CONFIG_NOSYSTEM").is_some(), "run tests with GIT_CONFIG_NOSYSTEM=1");
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> TempDir {
            let unique = NEXT_TEMP.fetch_add(1, Ordering::SeqCst);
            let path = env::temp_dir().join(format!("pull-repos-test-{}-{unique}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            TempDir(fs::canonicalize(path).unwrap())
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn git<I, S>(directory: &Path, args: I) -> String
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.com",
                "-c",
                "init.defaultBranch=main",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "protocol.file.allow=always",
            ])
            .arg("-C")
            .arg(directory)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(output.status.success(), "fixture git failed: {}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn write_executable(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    struct Repo {
        work: PathBuf,
        origin: PathBuf,
        checkout: PathBuf,
    }

    impl Repo {
        fn commit(directory: &Path, file: &str) {
            fs::write(directory.join(file), file).unwrap();
            git(directory, ["add", file]);
            git(directory, ["commit", "-q", "-m", file]);
        }

        fn remote_commit(&self, file: &str) {
            Repo::commit(&self.work, file);
            git(&self.work, ["push", "-q", "origin", "main"]);
        }

        fn local_commit(&self, file: &str) {
            Repo::commit(&self.checkout, file);
        }

        fn head(&self) -> String {
            git(&self.checkout, ["rev-parse", "HEAD"])
        }

        fn origin_head(&self) -> String {
            git(&self.origin, ["rev-parse", "main"])
        }

        fn config(&self, key: &str, value: &str) {
            git(&self.checkout, ["config", key, value]);
        }
    }

    struct RunOutput {
        code: u8,
        stdout: String,
        stderr: String,
        elapsed: Duration,
    }

    impl RunOutput {
        fn results(&self) -> BTreeMap<String, (String, String)> {
            self.stdout
                .lines()
                .skip(2)
                .take_while(|line| !line.is_empty())
                .map(|line| {
                    let (outcome, rest) = line.split_once(' ').unwrap();
                    let (_, rest) = rest.split_once("s  ").unwrap();
                    let (name, detail) = rest.split_once("  ").unwrap();
                    (name.to_string(), (outcome.to_string(), detail.to_string()))
                })
                .collect()
        }

        fn result(&self, name: &str) -> (String, String) {
            self.results().remove(name).unwrap_or_else(|| panic!("no result for {name}:\n{}", self.stdout))
        }

        fn summary(&self) -> Option<&str> {
            self.stdout.lines().find(|line| line.starts_with("checkouts "))
        }
    }

    struct Fixture {
        temp: TempDir,
        root: PathBuf,
        lock: PathBuf,
    }

    impl Fixture {
        fn new() -> Fixture {
            require_isolated_git_config();
            let temp = TempDir::new();
            let root = temp.0.join("root");
            fs::create_dir_all(&root).unwrap();
            let lock = temp.0.join(LOCK_FILE_NAME);
            Fixture { temp, root, lock }
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.temp.0.join(relative)
        }

        fn repo(&self, relative: &str) -> Repo {
            let slug = relative.replace('/', "_");
            let work = self.path(&format!("work/{slug}"));
            let origin = self.path(&format!("origins/{slug}.git"));
            let checkout = self.root.join(relative);
            fs::create_dir_all(&work).unwrap();
            git(&work, ["init", "-q", "-b", "main"]);
            Repo::commit(&work, "README");
            git(&self.temp.0, [OsStr::new("clone"), OsStr::new("-q"), OsStr::new("--bare"), work.as_os_str(), origin.as_os_str()]);
            git(&work, [OsStr::new("remote"), OsStr::new("add"), OsStr::new("origin"), origin.as_os_str()]);
            fs::create_dir_all(checkout.parent().unwrap()).unwrap();
            git(&self.temp.0, [OsStr::new("clone"), OsStr::new("-q"), origin.as_os_str(), checkout.as_os_str()]);
            Repo { work, origin, checkout }
        }

        fn script(&self, name: &str, body: &str) -> PathBuf {
            let path = self.path(&format!("scripts/{name}"));
            write_executable(&path, body);
            path
        }

        fn options(&self) -> Options {
            Options {
                root: self.root.clone(),
                jobs: 4,
                timeout: PhaseLimit::from_seconds(30.0),
                max_depth: 2,
                dry_run: false,
            }
        }

        fn run(&self, options: &Options) -> RunOutput {
            self.run_with_flag(options, &AtomicBool::new(false))
        }

        fn run_with_flag(&self, options: &Options, interrupted: &AtomicBool) -> RunOutput {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let started = Instant::now();
            let code = run(options, &self.lock, interrupted, &mut stdout, &mut stderr);
            RunOutput {
                code,
                stdout: String::from_utf8_lossy(&stdout).into_owned(),
                stderr: String::from_utf8_lossy(&stderr).into_owned(),
                elapsed: started.elapsed(),
            }
        }
    }

    fn process_is_gone(pid: libc::pid_t) -> bool {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if unsafe { libc::kill(pid, 0) } != 0 {
                return true;
            }
            thread::sleep(Duration::from_millis(50));
        }
        false
    }

    fn recorded_pids(path: &Path) -> Vec<libc::pid_t> {
        fs::read_to_string(path).unwrap().lines().map(|line| line.trim().parse().unwrap()).collect()
    }

    fn try_parse(values: &[&str]) -> Result<Options, clap::Error> {
        parse_cli(std::iter::once(PROGRAM).chain(values.iter().copied())).map(Options::from)
    }

    fn parsed(values: &[&str]) -> Options {
        try_parse(values).unwrap_or_else(|error| panic!("{values:?} was rejected: {error}"))
    }

    fn parse_error(values: &[&str]) -> clap::Error {
        match try_parse(values) {
            Ok(options) => panic!("{values:?} was accepted as {options:?}"),
            Err(error) => error,
        }
    }

    #[test]
    fn current_checkout_reports_current() {
        let fixture = Fixture::new();
        let repo = fixture.repo("app");
        let output = fixture.run(&fixture.options());
        assert_eq!(output.code, 0, "{}", output.stderr);
        assert_eq!(output.result("app"), ("current".to_string(), format!("main at {}", &repo.head()[..8])));
        assert_eq!(output.summary(), Some("checkouts 1  current 1  linked-worktrees-ignored 0"));
        assert!(output.stdout.starts_with(&format!("root {}  mode live  jobs 4  phase-timeout 30s  checkouts 1\n\n", fixture.root.display())));
    }

    #[test]
    fn new_remote_commits_fast_forward() {
        let fixture = Fixture::new();
        let repo = fixture.repo("app");
        let before = repo.head();
        repo.remote_commit("one");
        repo.remote_commit("two");
        let output = fixture.run(&fixture.options());
        assert_eq!(output.code, 0, "{}", output.stderr);
        let after = repo.origin_head();
        assert_eq!(repo.head(), after);
        assert_eq!(output.result("app"), ("updated".to_string(), format!("{}..{} (+2 commits)", &before[..8], &after[..8])));
    }

    #[test]
    fn dry_run_checks_only_and_takes_no_lock() {
        let fixture = Fixture::new();
        let repo = fixture.repo("app");
        repo.remote_commit("one");
        let tracking_before = git(&repo.checkout, ["rev-parse", "refs/remotes/origin/main"]);
        let held = OpenOptions::new().write(true).create(true).truncate(true).open(&fixture.lock).unwrap();
        held.try_lock().unwrap();
        let output = fixture.run(&Options { dry_run: true, ..fixture.options() });
        assert_eq!(output.code, 0, "{}", output.stderr);
        assert_eq!(output.result("app"), ("would-pull".to_string(), "main from origin/main".to_string()));
        assert_eq!(git(&repo.checkout, ["rev-parse", "refs/remotes/origin/main"]), tracking_before);
        assert!(output.stdout.contains("mode dry-run"));
    }

    #[test]
    fn live_run_stops_when_lock_is_held() {
        let fixture = Fixture::new();
        fixture.repo("app");
        let held = OpenOptions::new().write(true).create(true).truncate(true).open(&fixture.lock).unwrap();
        held.try_lock().unwrap();
        let output = fixture.run(&fixture.options());
        assert_eq!(output.code, 1);
        assert_eq!(output.stdout, "");
        assert_eq!(output.stderr, format!("another pull_repos run is active (lock: {})\n", fixture.lock.display()));
        drop(held);
        assert_eq!(fixture.run(&fixture.options()).code, 0);
    }

    #[test]
    fn ineligible_checkouts_are_skipped_without_changes() {
        let fixture = Fixture::new();
        let feature = fixture.repo("feature");
        git(&feature.checkout, ["switch", "-q", "-c", "topic"]);
        let detached = fixture.repo("detached");
        git(&detached.checkout, ["switch", "-q", "--detach"]);
        let no_remote = fixture.repo("no-remote");
        git(&no_remote.checkout, ["remote", "remove", "origin"]);
        let no_head = fixture.repo("no-origin-head");
        git(&no_head.checkout, ["remote", "set-head", "origin", "-d"]);
        let upstream = fixture.repo("upstream");
        upstream.config("branch.main.merge", "refs/heads/other");
        let tracked = fixture.repo("tracked");
        fs::write(tracked.checkout.join("README"), "changed").unwrap();
        let untracked = fixture.repo("untracked");
        fs::write(untracked.checkout.join("new.txt"), "new").unwrap();
        let ignored = fixture.repo("ignored");
        fs::write(ignored.checkout.join(".git/info/exclude"), "cache.txt\n").unwrap();
        fs::write(ignored.checkout.join("cache.txt"), "cache").unwrap();
        for marker in OPERATION_MARKERS {
            let repo = fixture.repo(&format!("op-{marker}"));
            fs::write(repo.checkout.join(".git").join(marker), "").unwrap();
        }
        let output = fixture.run(&Options { max_depth: 1, ..fixture.options() });
        assert_eq!(output.code, 0, "{}", output.stdout);
        let skipped = |detail: &str| ("skipped".to_string(), detail.to_string());
        assert_eq!(output.result("feature"), skipped("on branch topic, default branch is main"));
        assert_eq!(output.result("detached"), skipped("detached HEAD"));
        assert_eq!(output.result("no-remote"), skipped("remote origin not configured"));
        assert_eq!(output.result("no-origin-head"), skipped("default branch unknown (origin/HEAD not set)"));
        assert_eq!(output.result("upstream"), skipped("upstream of main is not origin/main"));
        assert_eq!(output.result("tracked"), skipped("working tree not clean (1 entries)"));
        assert_eq!(output.result("untracked"), skipped("working tree not clean (1 entries)"));
        assert_eq!(output.result("ignored").0, "current");
        for marker in OPERATION_MARKERS {
            assert_eq!(output.result(&format!("op-{marker}")), skipped(&format!("git operation in progress ({marker})")));
        }
        assert_eq!(fs::read_to_string(tracked.checkout.join("README")).unwrap(), "changed");
    }

    #[test]
    fn local_and_diverged_commits_are_kept() {
        let fixture = Fixture::new();
        let ahead = fixture.repo("ahead");
        ahead.local_commit("local");
        let diverged = fixture.repo("diverged");
        diverged.local_commit("local");
        diverged.remote_commit("remote");
        let ahead_before = ahead.head();
        let diverged_before = diverged.head();
        let output = fixture.run(&fixture.options());
        assert_eq!(output.code, 0);
        assert_eq!(output.result("ahead"), ("skipped".to_string(), "main has local commits not in origin/main".to_string()));
        assert_eq!(output.result("diverged"), ("skipped".to_string(), "main diverged from origin/main".to_string()));
        assert_eq!(ahead.head(), ahead_before);
        assert_eq!(diverged.head(), diverged_before);
    }

    #[test]
    fn merge_does_not_overwrite_ignored_files() {
        let fixture = Fixture::new();
        let repo = fixture.repo("app");
        fs::write(repo.checkout.join(".git/info/exclude"), "secret.txt\n").unwrap();
        fs::write(repo.checkout.join("secret.txt"), "local secret").unwrap();
        let before = repo.head();
        repo.remote_commit("secret.txt");
        let output = fixture.run(&fixture.options());
        assert_eq!(output.code, 1);
        let (outcome, detail) = output.result("app");
        assert_eq!(outcome, "failed");
        assert!(detail.starts_with("git merge failed: "), "{detail}");
        assert_eq!(fs::read_to_string(repo.checkout.join("secret.txt")).unwrap(), "local secret");
        assert_eq!(repo.head(), before);
    }

    #[test]
    fn discovery_respects_depth_symlinks_and_worktrees() {
        let fixture = Fixture::new();
        git(&fixture.root, ["init", "-q"]);
        let top = fixture.repo("top");
        fixture.repo("group/nested");
        fixture.repo("group/deeper/deepest");
        fixture.repo(".hidden/repo");
        fixture.repo("node_modules/repo");
        fixture.repo("group/node_modules/repo");
        fixture.repo("top/inner");
        fs::write(top.checkout.join(".git/info/exclude"), "inner/\n").unwrap();
        git(&top.checkout, ["worktree", "add", "-q", "../worktree"]);
        symlink(&top.checkout, fixture.root.join("link")).unwrap();
        fs::create_dir_all(fixture.root.join("gitlink")).unwrap();
        symlink(top.checkout.join(".git"), fixture.root.join("gitlink/.git")).unwrap();
        let submodule = fixture.repo("parent");
        let child = fixture.repo("child-source");
        git(&submodule.work, [OsStr::new("submodule"), OsStr::new("add"), OsStr::new("-q"), child.origin.as_os_str(), OsStr::new("module")]);
        git(&submodule.work, ["commit", "-q", "-m", "submodule"]);
        git(&submodule.work, ["push", "-q", "origin", "main"]);
        fs::remove_dir_all(&submodule.checkout).unwrap();
        git(&fixture.root, [OsStr::new("clone"), OsStr::new("-q"), OsStr::new("--recurse-submodules"), submodule.origin.as_os_str(), submodule.checkout.as_os_str()]);
        fs::remove_dir_all(&child.checkout).unwrap();
        let locked = fixture.root.join("locked");
        fs::create_dir_all(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

        let depth_two = fixture.run(&Options { dry_run: true, ..fixture.options() });
        let depth_three = fixture.run(&Options { dry_run: true, max_depth: 3, ..fixture.options() });
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

        let names = |output: &RunOutput| output.results().into_keys().collect::<Vec<_>>();
        assert_eq!(names(&depth_two), ["group/nested", "parent", "top"]);
        assert_eq!(names(&depth_three), ["group/deeper/deepest", "group/nested", "parent", "top"]);
        assert_eq!(depth_two.summary(), Some("checkouts 3  would-pull 3  linked-worktrees-ignored 1  unreadable-directories 1"));
        assert_eq!(depth_two.code, 0);
    }

    #[test]
    fn one_failing_repo_does_not_stop_others() {
        let fixture = Fixture::new();
        let broken = fixture.repo("broken");
        broken.config("remote.origin.url", fixture.path("missing.git").to_str().unwrap());
        let healthy = fixture.repo("healthy");
        healthy.remote_commit("one");
        let output = fixture.run(&fixture.options());
        assert_eq!(output.code, 1);
        let (outcome, detail) = output.result("broken");
        assert_eq!(outcome, "failed");
        assert!(detail.starts_with("git fetch failed: fatal: "), "{detail}");
        assert_eq!(output.result("healthy").0, "updated");
        assert_eq!(healthy.head(), healthy.origin_head());
        assert_eq!(output.summary(), Some("checkouts 2  updated 1  failed 1  linked-worktrees-ignored 0"));
    }

    #[test]
    fn fetch_timeout_terminates_process_group() {
        let fixture = Fixture::new();
        let repo = fixture.repo("app");
        let pids = fixture.path("term.pids");
        let pack = fixture.script("term-pack", &format!("echo $$ >> '{}'\nexec sleep 30", pids.display()));
        repo.config("remote.origin.uploadpack", pack.to_str().unwrap());
        let output = fixture.run(&Options { timeout: PhaseLimit::from_seconds(1.0), ..fixture.options() });
        assert_eq!(output.code, 1);
        assert_eq!(output.result("app"), ("failed".to_string(), "git fetch timed out, phase limit 1s".to_string()));
        assert!(output.elapsed < Duration::from_secs(3), "{:?}", output.elapsed);
        for pid in recorded_pids(&pids) {
            assert!(process_is_gone(pid), "process {pid} still runs");
        }
    }

    #[test]
    fn fetch_timeout_kills_processes_that_ignore_terminate() {
        let fixture = Fixture::new();
        let repo = fixture.repo("app");
        let pids = fixture.path("kill.pids");
        let pack = fixture.script(
            "kill-pack",
            &format!("trap '' TERM\necho $$ >> '{0}'\nsleep 30 &\necho $! >> '{0}'\nwait", pids.display()),
        );
        repo.config("remote.origin.uploadpack", pack.to_str().unwrap());
        let output = fixture.run(&Options { timeout: PhaseLimit::from_seconds(1.0), ..fixture.options() });
        assert_eq!(output.result("app"), ("failed".to_string(), "git fetch timed out, phase limit 1s".to_string()));
        assert!(output.elapsed >= Duration::from_secs(3), "{:?}", output.elapsed);
        assert!(output.elapsed < Duration::from_secs(6), "{:?}", output.elapsed);
        let pids = recorded_pids(&pids);
        assert_eq!(pids.len(), 2);
        for pid in pids {
            assert!(process_is_gone(pid), "process {pid} still runs");
        }
    }

    #[test]
    fn merge_timeout_reports_partial_update() {
        let fixture = Fixture::new();
        let repo = fixture.repo("app");
        repo.remote_commit("one");
        let hooks = fixture.path("slow-hooks");
        write_executable(&hooks.join("post-merge"), "sleep 30");
        repo.config("core.hooksPath", hooks.to_str().unwrap());
        let output = fixture.run(&Options { timeout: PhaseLimit::from_seconds(1.0), ..fixture.options() });
        assert_eq!(output.code, 1);
        assert_eq!(
            output.result("app"),
            ("failed".to_string(), "git merge timed out, phase limit 1s; checkout may be partly updated, run git status.".to_string())
        );
    }

    #[test]
    fn each_phase_gets_its_own_deadline() {
        let fixture = Fixture::new();
        let repo = fixture.repo("app");
        repo.remote_commit("one");
        let pack = fixture.script("slow-pack", "sleep 1.5\nexec git upload-pack \"$@\"");
        repo.config("remote.origin.uploadpack", pack.to_str().unwrap());
        let hooks = fixture.path("hooks");
        write_executable(&hooks.join("post-merge"), "sleep 1.5");
        repo.config("core.hooksPath", hooks.to_str().unwrap());
        let output = fixture.run(&Options { timeout: PhaseLimit::from_seconds(2.5), ..fixture.options() });
        assert_eq!(output.result("app").0, "updated", "{}", output.stdout);
        assert!(output.elapsed > Duration::from_secs_f64(2.5), "{:?}", output.elapsed);
    }

    #[test]
    fn command_is_not_started_after_phase_deadline() {
        let fixture = Fixture::new();
        let repo = fixture.repo("app");
        let git = RepoGit::new(&repo.checkout, PhaseLimit::from_seconds(0.001));
        thread::sleep(Duration::from_millis(20));
        match git.run(&["status"]) {
            Err(Stop::Timeout(message)) => assert_eq!(message, "git status not started, phase limit 0.001s reached"),
            Err(other) => panic!("unexpected error {other:?}"),
            Ok(_) => panic!("command started after deadline"),
        }
    }

    #[test]
    fn jobs_limit_parallel_repositories() {
        let fixture = Fixture::new();
        let active = fixture.path("active");
        fs::create_dir_all(&active).unwrap();
        let log = fixture.path("concurrency.log");
        let pack = fixture.script(
            "count-pack",
            &format!(
                "touch '{0}/'$$\nls '{0}' | wc -l >> '{1}'\nsleep 1\nrm '{0}/'$$\nexec git upload-pack \"$@\"",
                active.display(),
                log.display()
            ),
        );
        for name in ["a", "b", "c", "d"] {
            fixture.repo(name).config("remote.origin.uploadpack", pack.to_str().unwrap());
        }
        let output = fixture.run(&Options { jobs: 2, ..fixture.options() });
        assert_eq!(output.summary(), Some("checkouts 4  current 4  linked-worktrees-ignored 0"));
        let counts: Vec<usize> = fs::read_to_string(&log).unwrap().lines().map(|line| line.trim().parse().unwrap()).collect();
        assert_eq!(counts.len(), 4);
        assert_eq!(counts.iter().max(), Some(&2));
        assert!(output.elapsed >= Duration::from_secs(2), "{:?}", output.elapsed);
    }

    #[test]
    fn interrupt_finishes_in_flight_and_skips_queued() {
        let fixture = Fixture::new();
        let pack = fixture.script("slow-pack", "sleep 1\nexec git upload-pack \"$@\"");
        let repos: Vec<Repo> = ["a", "b", "c"]
            .iter()
            .map(|name| {
                let repo = fixture.repo(name);
                repo.remote_commit("one");
                repo.config("remote.origin.uploadpack", pack.to_str().unwrap());
                repo
            })
            .collect();
        let before: Vec<String> = repos.iter().map(Repo::head).collect();
        let interrupted = AtomicBool::new(false);
        let output = thread::scope(|scope| {
            scope.spawn(|| {
                thread::sleep(Duration::from_millis(300));
                interrupted.store(true, Ordering::SeqCst);
            });
            fixture.run_with_flag(&Options { jobs: 1, ..fixture.options() }, &interrupted)
        });
        assert_eq!(output.code, 130);
        assert_eq!(output.stderr, INTERRUPTED_MESSAGE);
        assert!(output.summary().is_none(), "{}", output.stdout);
        assert!(output.results().is_empty(), "{}", output.stdout);
        assert_eq!(repos[0].head(), repos[0].origin_head());
        assert_eq!(repos[1].head(), before[1]);
        assert_eq!(repos[2].head(), before[2]);
    }

    #[test]
    fn failure_detail_redacts_credentials_and_tolerates_invalid_utf8() {
        let fixture = Fixture::new();
        let repo = fixture.repo("app");
        let pack = fixture.script(
            "noisy-pack",
            "printf 'remote: hint\\nfatal: cannot read https://user:token@example.invalid/x.git \\377\\376\\n' >&2\nexit 1",
        );
        repo.config("remote.origin.uploadpack", pack.to_str().unwrap());
        let output = fixture.run(&fixture.options());
        assert_eq!(output.code, 1);
        let (outcome, detail) = output.result("app");
        assert_eq!(outcome, "failed");
        assert_eq!(detail, "git fetch failed: fatal: cannot read https://***@example.invalid/x.git \u{fffd}\u{fffd}");
    }

    #[test]
    fn root_must_be_a_directory() {
        let fixture = Fixture::new();
        let file = fixture.path("file");
        fs::write(&file, "").unwrap();
        let output = fixture.run(&Options { root: file.clone(), ..fixture.options() });
        assert_eq!(output.code, 1);
        assert_eq!(output.stderr, format!("root is not a directory: {}\n", file.display()));
        let missing = fixture.path("missing/child");
        let output = fixture.run(&Options { root: missing.clone(), ..fixture.options() });
        assert_eq!(output.stderr, format!("root is not a directory: {}\n", missing.display()));
    }

    #[test]
    fn empty_root_reports_zero_checkouts() {
        let fixture = Fixture::new();
        let output = fixture.run(&fixture.options());
        assert_eq!(output.code, 0);
        assert_eq!(output.summary(), Some("checkouts 0    linked-worktrees-ignored 0"));
    }

    #[test]
    fn cli_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn cli_defaults_and_values() {
        let defaults = parsed(&[]);
        assert_eq!(
            defaults,
            Options {
                root: PathBuf::from("~/Projects"),
                jobs: 4,
                timeout: PhaseLimit::from_seconds(120.0),
                max_depth: 2,
                dry_run: false,
            }
        );
        let options = parsed(&["--root", "/tmp/x", "--jobs=8", "--timeout", "0.5", "--max-depth=3", "--dry-run", "--jobs", "10"]);
        assert_eq!(options.root, PathBuf::from("/tmp/x"));
        assert_eq!(options.jobs, 10);
        assert_eq!(options.timeout.seconds, 0.5);
        assert_eq!(options.max_depth, 3);
        assert!(options.dry_run);
        assert_eq!(parsed(&["--root=/a", "--root", "/b"]).root, PathBuf::from("/b"));
        assert_eq!(parsed(&["--root", "-"]).root, PathBuf::from("-"));
        assert_eq!(parsed(&["--root", "-1"]).root, PathBuf::from("-1"));
        assert_eq!(parsed(&["--root", "-1.5"]).root, PathBuf::from("-1.5"));
        assert_eq!(parsed(&["--root=--"]).root, PathBuf::from("--"));
        assert_eq!(parsed(&["--root", "-.5"]).root, PathBuf::from("-.5"));
        assert_eq!(parsed(&["--root=-1e3"]).root, PathBuf::from("-1e3"));
        assert_eq!(parsed(&["--root", "~/Projects"]).root, PathBuf::from("~/Projects"));
        assert_eq!(parsed(&["--root", "/a", "--root", "-1"]).root, PathBuf::from("-1"));
        for (value, seconds) in [(" 5", 5.0), ("5 ", 5.0), ("\t5\n", 5.0), ("1_0", 10.0), ("1_0.2_5", 10.25), ("1e3", 1000.0), (".5", 0.5), ("5.", 5.0), ("+5", 5.0)] {
            assert_eq!(parsed(&["--timeout", value]).timeout.seconds, seconds, "{value:?}");
        }
        assert_eq!(parsed(&["--jobs", "99999999999999999999"]).jobs, usize::MAX);
    }

    #[test]
    fn cli_help_goes_to_stdout_with_exit_zero() {
        for values in [&["-h"][..], &["--help"], &["--dry-run", "-h"], &["-h", "--"]] {
            let error = parse_error(values);
            assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp, "{values:?}");
            assert!(!error.use_stderr(), "{values:?}");
            assert_eq!(error.exit_code(), 0, "{values:?}");
        }
        let help = Cli::command().render_help().to_string();
        for text in ["Usage: pull_repos.rs [OPTIONS]", "[default: ~/Projects]", "[default: 4]", "[default: 120]", "[default: 2]", "--dry-run", "Exit status:", "130  interrupted"] {
            assert!(help.contains(text), "help lacks {text:?}:\n{help}");
        }
    }

    #[test]
    fn cli_rejects_invalid_values() {
        for values in [
            &["--jobs", "0"][..],
            &["--jobs", "-1"],
            &["--jobs", "x"],
            &["--jobs", ""],
            &["--jobs", "1.5"],
            &["--jobs", " 4"],
            &["--jobs", "1_0"],
            &["--max-depth", "0"],
            &["--max-depth", "-1"],
            &["--timeout", "0"],
            &["--timeout", "-1"],
            &["--timeout", "x"],
            &["--timeout", "nan"],
            &["--timeout", "inf"],
            &["--timeout", "1__0"],
            &["--timeout", "_1"],
            &["--timeout", "1_"],
            &["--timeout", "1_.5"],
            &["--timeout", " "],
            &["--jobs"],
            &["--root"],
            &["--root", ""],
            &["--root="],
            &["--root", "--dry-run"],
            &["--dry"],
            &["--dry-run=yes"],
            &["--bogus", "y"],
            &["extra"],
            &["--"],
            &["--jobs", "2", "--"],
            &["--dry-run", "--"],
            &["--root", "-1."],
            &["--root", "-1e3"],
            &["--root", "-1.5.5"],
            &["--root", "-h"],
            &["--root", "-x"],
            &["--root", "-1e3", "--root", "/a"],
        ] {
            let error = parse_error(values);
            assert!(error.use_stderr(), "{values:?}");
            assert_eq!(error.exit_code(), 2, "{values:?}");
        }
        assert!(parse_error(&["--jobs", "0"]).to_string().contains("must be at least 1"));
        assert!(parse_error(&["--timeout", "inf"]).to_string().contains("must be a finite number greater than 0"));
        assert!(parse_error(&["--dry"]).to_string().contains("--dry-run"));
        assert_eq!(parse_error(&["--dry-run", "--"]).kind(), clap::error::ErrorKind::UnknownArgument);
    }

    #[test]
    fn redact_hides_url_credentials() {
        assert_eq!(redact("fatal: https://user:pw@host/repo and ssh://tok@h"), "fatal: https://***@host/repo and ssh://***@h");
        assert_eq!(redact("https://host/a@b"), "https://host/a@b");
        assert_eq!(redact(":///@x"), ":///@x");
    }

    #[test]
    fn error_summary_prefers_first_error_line() {
        assert_eq!(error_summary("  hint\r\nerror: one\nfatal: two\n"), "error: one");
        assert_eq!(error_summary("\n  plain line \n"), "plain line");
        assert_eq!(error_summary(""), "");
        assert_eq!(error_summary(&format!("fatal: {}", "x".repeat(300))).chars().count(), 200);
    }

    #[test]
    fn format_g_matches_python() {
        for (value, expected) in [
            (120.0, "120"),
            (0.5, "0.5"),
            (1.0, "1"),
            (0.001, "0.001"),
            (0.00001, "1e-05"),
            (1234567.0, "1.23457e+06"),
            (999999.5, "1e+06"),
            (2.25, "2.25"),
            (123456.0, "123456"),
        ] {
            assert_eq!(format_g(value), expected);
        }
    }

    #[test]
    fn split_lines_matches_python() {
        assert_eq!(python_split_lines("a\r\nb\rc\nd\x0ce"), ["a", "b", "c", "d", "e"]);
        assert_eq!(python_split_lines("a\n\nb\n"), ["a", "", "b"]);
    }

    #[test]
    fn parent_of_symlinked_root_selects_parent_of_link_target() {
        let fixture = Fixture::new();
        fixture.repo("a/sibling-repo");
        fixture.repo("top-repo");
        fs::create_dir_all(fixture.root.join("a/b")).unwrap();
        symlink(fixture.root.join("a/b"), fixture.root.join("link")).unwrap();
        let options = Options { root: fixture.root.join("link/.."), dry_run: true, ..fixture.options() };
        let output = fixture.run(&options);
        assert_eq!(output.code, 0, "{}", output.stderr);
        assert!(output.stdout.starts_with(&format!("root {}  mode dry-run", fixture.root.join("a").display())), "{}", output.stdout);
        assert_eq!(output.results().into_keys().collect::<Vec<_>>(), ["sibling-repo"]);
    }

    #[test]
    fn resolve_root_follows_python_realpath() {
        let fixture = Fixture::new();
        let root = &fixture.root;
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::write(root.join("file"), "").unwrap();
        symlink(root.join("a/b"), root.join("link")).unwrap();
        symlink("a/../a/b", root.join("relative-link")).unwrap();
        symlink("link", root.join("chain")).unwrap();
        symlink("loop", root.join("loop")).unwrap();
        symlink("missing-target", root.join("dangling")).unwrap();
        for (input, expected) in [
            ("a/./b/../", "a"),
            ("link", "a/b"),
            ("link/..", "a"),
            ("relative-link/..", "a"),
            ("chain/..", "a"),
            ("link/../../link/..", "a"),
            ("missing/child", "missing/child"),
            ("missing/../a", "a"),
            ("missing/../link/..", "a"),
            ("file/..", ""),
            ("loop", "loop"),
            ("dangling/..", ""),
            ("dangling/child", "missing-target/child"),
        ] {
            let expected = if expected.is_empty() { root.clone() } else { root.join(expected) };
            assert_eq!(resolve_root(&root.join(input)).unwrap(), expected, "input {input}");
        }
        assert_eq!(resolve_root(Path::new("/..")).unwrap(), Path::new("/"));
        assert_eq!(resolve_root(Path::new("")).unwrap(), env::current_dir().unwrap());
    }
}
