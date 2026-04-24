use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::IsTerminal;
use std::path::Path;

use serde_json::Value;

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

fn format_value(value: &Value) -> String {
    match value {
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        }
        _ => value.to_string(),
    }
}

fn push_diff(
    diffs: &mut Vec<DiffEntry>,
    path: &[PathSegment],
    old: Option<&Value>,
    new: Option<&Value>,
) {
    diffs.push(DiffEntry {
        path: path.to_vec(),
        old: old.cloned(),
        new: new.cloned(),
    });
}

fn diff_values(path: &mut Vec<PathSegment>, old: &Value, new: &Value, diffs: &mut Vec<DiffEntry>) {
    if old == new {
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
                    (Some(av), Some(bv)) => diff_values(path, av, bv, diffs),
                    (Some(av), None) => push_diff(diffs, path, Some(av), None),
                    (None, Some(bv)) => push_diff(diffs, path, None, Some(bv)),
                    (None, None) => {}
                }
                path.pop();
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            let shared = a.len().min(b.len());

            for i in 0..shared {
                path.push(PathSegment::Index(i));
                diff_values(path, &a[i], &b[i], diffs);
                path.pop();
            }

            for (i, value) in a.iter().enumerate().skip(shared) {
                path.push(PathSegment::Index(i));
                push_diff(diffs, path, Some(value), None);
                path.pop();
            }

            for (i, value) in b.iter().enumerate().skip(shared) {
                path.push(PathSegment::Index(i));
                push_diff(diffs, path, None, Some(value));
                path.pop();
            }
        }
        _ => push_diff(diffs, path, Some(old), Some(new)),
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

fn print_diff_entry(entry: &DiffEntry, colors: Colors) {
    let path = format_path(&entry.path);
    if colors.enabled {
        println!("{}@ {}{}", Colors::HEADER, path, Colors::RESET);
    } else {
        println!("@ {}", path);
    }

    if let Some(old) = &entry.old {
        let rendered = format_value(old);
        print_prefixed('-', &rendered, Colors::REMOVED, colors);
    }
    if let Some(new) = &entry.new {
        let rendered = format_value(new);
        print_prefixed('+', &rendered, Colors::ADDED, colors);
    }
    println!();
}

fn read_json_file(path: &Path) -> Result<Value, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read '{}': {}", path.display(), err))?;
    serde_json::from_str(&raw)
        .map_err(|err| format!("failed to parse JSON from '{}': {}", path.display(), err))
}

fn usage(bin: &str) {
    eprintln!("Usage: {} <old.json> <new.json>", bin);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        usage(args.get(0).map(String::as_str).unwrap_or("json-diff"));
        std::process::exit(2);
    }

    let old_path = Path::new(&args[1]);
    let new_path = Path::new(&args[2]);

    let old = match read_json_file(old_path) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    };

    let new = match read_json_file(new_path) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    };

    let mut diffs = Vec::new();
    let mut path = Vec::new();
    diff_values(&mut path, &old, &new, &mut diffs);
    let colors = Colors::new(std::io::stdout().is_terminal());

    for entry in &diffs {
        print_diff_entry(entry, colors);
    }
}
