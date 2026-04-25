use std::collections::BTreeSet;
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use serde_json::Value;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ColorChoice {
    Auto,
    On,
    Off,
}

impl ColorChoice {
    fn enabled(self) -> bool {
        match self {
            Self::Auto => std::io::stdout().is_terminal(),
            Self::On => true,
            Self::Off => false,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum PrettyChoice {
    On,
    Off,
}

impl PrettyChoice {
    fn enabled(self) -> bool {
        matches!(self, Self::On)
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum StatsChoice {
    On,
    Off,
}

impl StatsChoice {
    fn enabled(self) -> bool {
        matches!(self, Self::On)
    }
}

#[derive(Debug, Parser)]
#[command(version, about = "Diff two JSON files")]
struct Args {
    old: PathBuf,
    new: PathBuf,

    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    color: ColorChoice,

    #[arg(long, value_enum, default_value_t = PrettyChoice::On)]
    pretty: PrettyChoice,

    #[arg(long, value_enum, default_value_t = StatsChoice::On)]
    stats: StatsChoice,

    #[arg(long)]
    ignore_store_path_hashes: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct CompareOptions {
    ignore_store_path_hashes: bool,
}

#[derive(Clone, Copy, Debug)]
struct PrintOptions {
    pretty: bool,
}

#[derive(Clone, Debug)]
enum PathSegment {
    Key(String),
    Index(usize),
}

#[derive(Clone, Debug)]
struct DiffEntry {
    path: Vec<PathSegment>,
    old: Option<Value>,
    new: Option<Value>,
}

#[derive(Clone, Copy, Debug, Default)]
struct DiffStats {
    added: usize,
    deleted: usize,
    ignored: usize,
}

impl DiffStats {
    fn record_diff(&mut self, old: Option<&Value>, new: Option<&Value>) {
        if old.is_some() {
            self.deleted += 1;
        }
        if new.is_some() {
            self.added += 1;
        }
    }
}

#[derive(Clone, Copy)]
struct Colors {
    enabled: bool,
}

impl Colors {
    const RESET: &'static str = "\x1b[0m";
    const HEADER: &'static str = "\x1b[1;36m";
    const REMOVED: &'static str = "\x1b[31m";
    const ADDED: &'static str = "\x1b[32m";

    fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

fn format_path(path: &[PathSegment]) -> String {
    if path.is_empty() {
        return "<root>".to_string();
    }

    let mut out = String::new();
    for segment in path {
        match segment {
            PathSegment::Key(key) => {
                if !out.is_empty() {
                    out.push('.');
                }
                out.push_str(key);
            }
            PathSegment::Index(index) => {
                out.push('[');
                out.push_str(&index.to_string());
                out.push(']');
            }
        }
    }
    out
}

fn format_value(value: &Value, options: PrintOptions) -> String {
    if options.pretty {
        match value {
            Value::Array(_) | Value::Object(_) => {
                serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
            }
            _ => value.to_string(),
        }
    } else {
        value.to_string()
    }
}

fn push_diff(
    diffs: &mut Vec<DiffEntry>,
    stats: &mut DiffStats,
    path: &[PathSegment],
    old: Option<&Value>,
    new: Option<&Value>,
) {
    stats.record_diff(old, new);
    diffs.push(DiffEntry {
        path: path.to_vec(),
        old: old.cloned(),
        new: new.cloned(),
    });
}

fn is_nix_hash_byte(byte: u8) -> bool {
    byte.is_ascii_digit() || byte.is_ascii_lowercase()
}

fn has_nix_store_hash_at(bytes: &[u8], index: usize) -> bool {
    const PREFIX: &[u8] = b"/nix/store/";
    const HASH_LEN: usize = 32;

    let Some(after_prefix) = index.checked_add(PREFIX.len()) else {
        return false;
    };
    let Some(after_hash) = after_prefix.checked_add(HASH_LEN) else {
        return false;
    };

    bytes
        .get(index..after_prefix)
        .is_some_and(|prefix| prefix == PREFIX)
        && bytes.get(after_hash) == Some(&b'-')
        && bytes[after_prefix..after_hash]
            .iter()
            .all(|byte| is_nix_hash_byte(*byte))
}

fn strings_equal_ignoring_nix_store_hashes(a: &str, b: &str) -> bool {
    const PREFIX_LEN: usize = b"/nix/store/".len();
    const HASH_LEN: usize = 32;

    let a = a.as_bytes();
    let b = b.as_bytes();
    let mut a_index = 0;
    let mut b_index = 0;

    while a_index < a.len() && b_index < b.len() {
        if has_nix_store_hash_at(a, a_index) && has_nix_store_hash_at(b, b_index) {
            a_index += PREFIX_LEN + HASH_LEN;
            b_index += PREFIX_LEN + HASH_LEN;
        } else if a[a_index] == b[b_index] {
            a_index += 1;
            b_index += 1;
        } else {
            return false;
        }
    }

    a_index == a.len() && b_index == b.len()
}

fn values_equal(old: &Value, new: &Value, options: CompareOptions) -> bool {
    if old == new {
        return true;
    }

    match (old, new) {
        (Value::String(a), Value::String(b)) if options.ignore_store_path_hashes => {
            strings_equal_ignoring_nix_store_hashes(a, b)
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(old, new)| values_equal(old, new, options))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter().all(|(key, old)| {
                    b.get(key)
                        .is_some_and(|new| values_equal(old, new, options))
                })
        }
        _ => false,
    }
}

fn lcs_table(a: &[Value], b: &[Value], options: CompareOptions) -> Vec<Vec<usize>> {
    let mut dp = vec![vec![0; b.len() + 1]; a.len() + 1];

    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            dp[i][j] = if values_equal(&a[i], &b[j], options) {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    dp
}

fn diff_arrays(
    path: &mut Vec<PathSegment>,
    old: &[Value],
    new: &[Value],
    diffs: &mut Vec<DiffEntry>,
    stats: &mut DiffStats,
    options: CompareOptions,
) {
    let dp = lcs_table(old, new, options);

    let mut i = 0;
    let mut j = 0;

    while i < old.len() || j < new.len() {
        if i < old.len() && j < new.len() && old[i] == new[j] {
            i += 1;
            j += 1;
        } else if i < old.len() && j < new.len() && values_equal(&old[i], &new[j], options) {
            count_ignored_diffs(&old[i], &new[j], stats, options);
            i += 1;
            j += 1;
        } else if j < new.len() && (i == old.len() || dp[i][j + 1] >= dp[i + 1][j]) {
            path.push(PathSegment::Index(j));
            push_diff(diffs, stats, path, None, Some(&new[j]));
            path.pop();
            j += 1;
        } else if i < old.len() {
            path.push(PathSegment::Index(i));
            push_diff(diffs, stats, path, Some(&old[i]), None);
            path.pop();
            i += 1;
        }
    }
}

fn diff_values(
    path: &mut Vec<PathSegment>,
    old: &Value,
    new: &Value,
    diffs: &mut Vec<DiffEntry>,
    stats: &mut DiffStats,
    options: CompareOptions,
) {
    if old == new {
        return;
    }

    if values_equal(old, new, options) {
        count_ignored_diffs(old, new, stats, options);
        return;
    }

    match (old, new) {
        (Value::Object(a), Value::Object(b)) => {
            let mut keys: BTreeSet<&str> = BTreeSet::new();
            for key in a.keys() {
                keys.insert(key.as_str());
            }
            for key in b.keys() {
                keys.insert(key.as_str());
            }

            for key in keys {
                path.push(PathSegment::Key(key.to_string()));
                match (a.get(key), b.get(key)) {
                    (Some(av), Some(bv)) => diff_values(path, av, bv, diffs, stats, options),
                    (Some(av), None) => push_diff(diffs, stats, path, Some(av), None),
                    (None, Some(bv)) => push_diff(diffs, stats, path, None, Some(bv)),
                    (None, None) => {}
                }
                path.pop();
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            diff_arrays(path, a, b, diffs, stats, options);
        }
        _ => push_diff(diffs, stats, path, Some(old), Some(new)),
    }
}

fn count_ignored_diffs(old: &Value, new: &Value, stats: &mut DiffStats, options: CompareOptions) {
    if old == new {
        return;
    }

    match (old, new) {
        (Value::String(a), Value::String(b))
            if options.ignore_store_path_hashes
                && strings_equal_ignoring_nix_store_hashes(a, b) =>
        {
            stats.ignored += 1;
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (old, new) in a.iter().zip(b) {
                count_ignored_diffs(old, new, stats, options);
            }
        }
        (Value::Object(a), Value::Object(b)) if a.len() == b.len() => {
            for (key, old) in a {
                if let Some(new) = b.get(key) {
                    count_ignored_diffs(old, new, stats, options);
                }
            }
        }
        _ => {}
    }
}

fn print_prefixed(prefix: char, text: &str, color_code: &'static str, colors: Colors) {
    for line in text.lines() {
        if colors.enabled {
            println!("{}{} {}{}", color_code, prefix, line, Colors::RESET);
        } else {
            println!("{} {}", prefix, line);
        }
    }
}

fn print_diff_entry(entry: &DiffEntry, colors: Colors, options: PrintOptions) {
    let path = format_path(&entry.path);
    if colors.enabled {
        println!("{}@ {}{}", Colors::HEADER, path, Colors::RESET);
    } else {
        println!("@ {}", path);
    }

    if let Some(old) = &entry.old {
        let rendered = format_value(old, options);
        print_prefixed('-', &rendered, Colors::REMOVED, colors);
    }
    if let Some(new) = &entry.new {
        let rendered = format_value(new, options);
        print_prefixed('+', &rendered, Colors::ADDED, colors);
    }
    println!();
}

fn print_stats(stats: DiffStats) {
    println!(
        "Stats: added {}, deleted {}, ignored {}",
        stats.added, stats.deleted, stats.ignored
    );
}

fn read_json_file(path: &Path) -> Result<Value> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read '{}'", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse JSON from '{}'", path.display()))
}

fn main() -> Result<()> {
    let args = Args::parse();

    let old = read_json_file(&args.old)?;
    let new = read_json_file(&args.new)?;
    let options = CompareOptions {
        ignore_store_path_hashes: args.ignore_store_path_hashes,
    };

    let mut diffs = Vec::new();
    let mut stats = DiffStats::default();
    let mut path = Vec::new();
    diff_values(&mut path, &old, &new, &mut diffs, &mut stats, options);
    let colors = Colors::new(args.color.enabled());
    let print_options = PrintOptions {
        pretty: args.pretty.enabled(),
    };

    for entry in &diffs {
        print_diff_entry(entry, colors, print_options);
    }

    if args.stats.enabled() {
        print_stats(stats);
    }

    Ok(())
}
