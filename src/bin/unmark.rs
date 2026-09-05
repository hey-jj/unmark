//! Thin CLI: parse arguments, call the library, map exits. stdout carries the
//! machine-readable JSON result, one object per asset. Diagnostics go to
//! stderr. `clean` writes each cleaned asset under --out only when its run
//! may write.

use lexopt::prelude::*;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use unmark::report::{
    self, EXIT_INSTRUMENTATION, EXIT_OK, EXIT_SANITY, EXIT_UNSUPPORTED, EXIT_USAGE,
};
use unmark::{clean, inspect, plan, policy, skill, Options, UnmarkError, VerifyOutcome};

const DEFAULT_MAX_BYTES: usize = 64 * 1024 * 1024;

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("unmark: {e}");
            ExitCode::from(EXIT_USAGE as u8)
        }
    }
}

fn run() -> Result<i32, lexopt::Error> {
    let mut parser = lexopt::Parser::from_env();
    match parser.next()? {
        Some(Value(cmd)) => {
            let cmd = cmd.string()?;
            match cmd.as_str() {
                "inspect" => cmd_survey(parser, false),
                "plan" => cmd_survey(parser, true),
                "clean" => cmd_clean(parser),
                "verify" => cmd_verify(parser),
                "policy" => cmd_policy(parser),
                other => {
                    eprintln!("unmark: unknown subcommand {other}");
                    Ok(EXIT_USAGE)
                }
            }
        }
        Some(Long("version")) | Some(Short('V')) => {
            println!("unmark {}", env!("CARGO_PKG_VERSION"));
            Ok(EXIT_OK)
        }
        Some(Long("help")) | Some(Short('h')) => {
            println!("{}", usage());
            Ok(EXIT_OK)
        }
        None => {
            eprintln!("{}", usage());
            Ok(EXIT_USAGE)
        }
        Some(arg) => Err(arg.unexpected()),
    }
}

fn usage() -> &'static str {
    "usage:\n  \
     unmark inspect [--keep <id|class>]... [--no-degrade] [--strip-capture] [--output json|text] [--max-bytes N] PATH...\n  \
     unmark plan    [same flags] PATH...\n  \
     unmark clean   --out PATH|DIR [--overwrite] [--keep <id|class>]... [--no-degrade] [--strip-capture]\n                 \
     [--output json|text] [--max-bytes N] PATH...\n  \
     unmark verify  --report FILE PATH\n  \
     unmark policy  digest | show | snapshot [--out FILE]\n  \
     unmark --version | -V | --help | -h\n\
     \n\
     Each file receives one default run that strips every mark it finds. A directory PATH\n\
     processes every file under it. --out names a file for one input, or a directory for\n\
     several; the extension must match the emitted container.\n\
     \n\
     exit codes:\n  \
     0   the run completed\n  \
     2   usage error\n  \
     10  a confirmable mark remains\n  \
     30  measurement, decoding, or required inspection failed\n  \
     40  unsupported input\n  \
     50  the sanity floor refused the operation; nothing was written"
}

struct Common {
    output_text: bool,
    max_bytes: usize,
    paths: Vec<String>,
    opts: Options,
    out: Option<String>,
    overwrite: bool,
    report_path: Option<String>,
}

fn parse_common(parser: &mut lexopt::Parser) -> Result<Common, lexopt::Error> {
    let mut c = Common {
        output_text: false,
        max_bytes: DEFAULT_MAX_BYTES,
        paths: Vec::new(),
        opts: Options::default(),
        out: None,
        overwrite: false,
        report_path: None,
    };
    while let Some(arg) = parser.next()? {
        match arg {
            Long("output") => c.output_text = matches!(parser.value()?.string()?.as_str(), "text"),
            Long("max-bytes") => c.max_bytes = parser.value()?.parse()?,
            Long("keep") => c.opts.keep.push(parser.value()?.string()?),
            Long("no-degrade") => c.opts.no_degrade = true,
            Long("strip-capture") => c.opts.strip_capture = true,
            Long("out") => c.out = Some(parser.value()?.string()?),
            Long("overwrite") => c.overwrite = true,
            Long("report") => c.report_path = Some(parser.value()?.string()?),
            Long("help") | Short('h') => {
                println!("{}", usage());
                std::process::exit(EXIT_OK);
            }
            Value(v) => c.paths.push(v.string()?),
            arg => return Err(arg.unexpected()),
        }
    }
    Ok(c)
}

/// One input: its path and, for a file found under a directory argument,
/// its path relative to that directory, so a batch mirrors the tree under
/// --out and two files with one name in different directories never
/// collide.
struct Input {
    path: PathBuf,
    relative: Option<PathBuf>,
}

fn walk_dir(
    dir: &Path,
    root: &Path,
    prefix: Option<&Path>,
    out: &mut Vec<Input>,
) -> Result<(), String> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            walk_dir(&entry, root, prefix, out)?;
        } else if entry.is_file() {
            let relative = entry.strip_prefix(root).ok().map(|r| match prefix {
                Some(p) => p.join(r),
                None => r.to_path_buf(),
            });
            out.push(Input {
                path: entry,
                relative,
            });
        }
    }
    Ok(())
}

/// Expand directory arguments into their files, recursively and sorted, so
/// a batch runs in a fixed order. One directory mirrors its tree straight
/// under --out; several mirror under their own names so equal paths in two
/// trees never collide. Returns the inputs and whether any argument was a
/// directory.
fn expand_inputs(paths: &[String]) -> Result<(Vec<Input>, bool), String> {
    let mut out = Vec::new();
    let mut any_dir = false;
    let dir_count = paths.iter().filter(|p| Path::new(p).is_dir()).count();
    for p in paths {
        if p == "-" {
            out.push(Input {
                path: PathBuf::from("-"),
                relative: None,
            });
            continue;
        }
        let path = Path::new(p);
        if path.is_dir() {
            any_dir = true;
            let prefix = if dir_count > 1 {
                path.file_name().map(Path::new)
            } else {
                None
            };
            walk_dir(path, path, prefix, &mut out)?;
        } else if path.is_file() {
            out.push(Input {
                path: path.to_path_buf(),
                relative: None,
            });
        } else {
            return Err(format!("{p}: no such file or directory"));
        }
    }
    if out.is_empty() {
        return Err("no input path given".to_string());
    }
    Ok((out, any_dir))
}

fn read_input(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    let bytes = if path == Path::new("-") {
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .map_err(|e| format!("stdin read: {e}"))?;
        buf
    } else {
        std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?
    };
    if bytes.len() > max {
        return Err(format!(
            "{} is {} bytes, over the {max}-byte limit; raise --max-bytes to process it",
            path.display(),
            bytes.len()
        ));
    }
    Ok(bytes)
}

fn emit(report_text: bool, r: &report::Report) {
    if report_text {
        print!("{}", report::render_text(r));
    } else {
        match serde_json::to_string(r) {
            Ok(s) => println!("{s}"),
            Err(e) => eprintln!("unmark: serialize: {e}"),
        }
    }
}

/// Combine two exit codes, keeping the more serious.
fn worst(a: i32, b: i32) -> i32 {
    fn rank(c: i32) -> i32 {
        match c {
            EXIT_USAGE => 6,
            EXIT_UNSUPPORTED => 5,
            EXIT_INSTRUMENTATION => 4,
            EXIT_SANITY => 3,
            report::EXIT_MARK_REMAINS => 2,
            _ => 0,
        }
    }
    if rank(b) > rank(a) {
        b
    } else {
        a
    }
}

fn load_policy() -> Result<policy::PolicyPackage, i32> {
    policy::load().map_err(|e| {
        eprintln!("unmark: policy load failed: {e}");
        EXIT_INSTRUMENTATION
    })
}

/// A failed run still yields one report for its input, with the message
/// and the exit code, so a batch never loses a file's outcome.
fn emit_failure(
    text: bool,
    pkg: &policy::PolicyPackage,
    verb: unmark::Verb,
    path: &Path,
    bytes: &[u8],
    message: &str,
    code: i32,
) {
    eprintln!("unmark: {}: {message}", path.display());
    let mut r = unmark::failure_report(pkg, verb, bytes, message, code);
    r.input = Some(path.display().to_string());
    emit(text, &r);
}

fn error_message(e: &UnmarkError) -> (String, i32) {
    match e {
        UnmarkError::Usage(m) => (m.clone(), EXIT_USAGE),
        UnmarkError::Unsupported(m) => (format!("unsupported_input: {m}"), EXIT_UNSUPPORTED),
        UnmarkError::Inspection(m) => (format!("inspection_failed: {m}"), EXIT_INSTRUMENTATION),
    }
}

fn cmd_survey(mut parser: lexopt::Parser, is_plan: bool) -> Result<i32, lexopt::Error> {
    let c = parse_common(&mut parser)?;
    let (inputs, _) = match expand_inputs(&c.paths) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("unmark: {e}");
            return Ok(EXIT_USAGE);
        }
    };
    let pkg = match load_policy() {
        Ok(p) => p,
        Err(code) => return Ok(code),
    };
    let verb = if is_plan {
        unmark::Verb::Plan
    } else {
        unmark::Verb::Inspect
    };
    let mut exit = EXIT_OK;
    for input in inputs {
        let path = &input.path;
        let bytes = match read_input(path, c.max_bytes) {
            Ok(b) => b,
            Err(e) => {
                emit_failure(c.output_text, &pkg, verb, path, &[], &e, EXIT_UNSUPPORTED);
                exit = worst(exit, EXIT_UNSUPPORTED);
                continue;
            }
        };
        let result = if is_plan {
            plan(&bytes, &c.opts, &pkg)
        } else {
            inspect(&bytes, &c.opts, &pkg)
        };
        match result {
            Ok(mut r) => {
                r.input = Some(path.display().to_string());
                emit(c.output_text, &r);
                exit = worst(exit, r.exit_code);
            }
            Err(e) => {
                let (m, code) = error_message(&e);
                emit_failure(c.output_text, &pkg, verb, path, &bytes, &m, code);
                exit = worst(exit, code);
            }
        }
    }
    Ok(exit)
}

/// The extensions a written container may carry, by report format name.
fn extensions_for_output(format: &str) -> Option<&'static [&'static str]> {
    match format {
        "jpeg" => Some(&["jpg", "jpeg"]),
        "png" => Some(&["png"]),
        "webp" => Some(&["webp"]),
        "riff-wav" => Some(&["wav"]),
        "flac" => Some(&["flac"]),
        "isobmff" => Some(&["mp4", "m4a", "mov"]),
        "mp3" => Some(&["mp3"]),
        _ => None,
    }
}

/// True when two paths name one file: the same canonical path, or on Unix
/// the same device and inode, which covers a symlink and a hard link.
fn same_file(a: &Path, b: &Path) -> bool {
    if let (Ok(ca), Ok(cb)) = (a.canonicalize(), b.canonicalize()) {
        if ca == cb {
            return true;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(ma), Ok(mb)) = (std::fs::metadata(a), std::fs::metadata(b)) {
            return ma.dev() == mb.dev() && ma.ino() == mb.ino();
        }
    }
    false
}

/// Write through a temporary file beside the destination and rename it into
/// place, so a failed write leaves no partial output and a replacement is
/// one atomic step.
fn write_atomic(dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    let tmp = dir.join(format!(".{name}.unmark-{}.tmp", std::process::id()));
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, dest)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn cmd_clean(mut parser: lexopt::Parser) -> Result<i32, lexopt::Error> {
    let c = parse_common(&mut parser)?;
    let Some(out) = c.out.clone() else {
        eprintln!("unmark: clean requires --out PATH or --out DIR. Keeping the input is the backup, so there is no in-place mode.");
        return Ok(EXIT_USAGE);
    };
    let (inputs, any_dir) = match expand_inputs(&c.paths) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("unmark: {e}");
            return Ok(EXIT_USAGE);
        }
    };
    let out_path = Path::new(&out);
    let batch = any_dir || inputs.len() > 1 || out_path.is_dir() || out.ends_with('/');
    if batch && out_path.exists() && !out_path.is_dir() {
        eprintln!("unmark: several inputs need --out to be a directory");
        return Ok(EXIT_USAGE);
    }
    if batch && !out_path.exists() {
        if let Err(e) = std::fs::create_dir_all(out_path) {
            eprintln!("unmark: {out}: {e}");
            return Ok(EXIT_USAGE);
        }
    }
    let pkg = match load_policy() {
        Ok(p) => p,
        Err(code) => return Ok(code),
    };
    let mut exit = EXIT_OK;
    let mut written: Vec<PathBuf> = Vec::new();
    for input in inputs {
        let path = &input.path;
        // The input is never the output. A single --out that names the
        // input, through a symlink or a hard link too, is refused before
        // any work.
        if !batch && same_file(path, out_path) {
            eprintln!(
                "unmark: --out {} names the input {}. Keeping the input is the backup, so nothing is written.",
                out_path.display(),
                path.display()
            );
            return Ok(EXIT_USAGE);
        }
        let bytes = match read_input(path, c.max_bytes) {
            Ok(b) => b,
            Err(e) => {
                emit_failure(
                    c.output_text,
                    &pkg,
                    unmark::Verb::Clean,
                    path,
                    &[],
                    &e,
                    EXIT_UNSUPPORTED,
                );
                exit = worst(exit, EXIT_UNSUPPORTED);
                continue;
            }
        };
        let outcome = match clean(&bytes, &c.opts, &pkg) {
            Ok(o) => o,
            Err(e) => {
                let (m, code) = error_message(&e);
                emit_failure(
                    c.output_text,
                    &pkg,
                    unmark::Verb::Clean,
                    path,
                    &bytes,
                    &m,
                    code,
                );
                exit = worst(exit, code);
                continue;
            }
        };
        let mut r = outcome.report;
        r.input = Some(path.display().to_string());
        let Some(out_bytes) = outcome.output else {
            emit(c.output_text, &r);
            exit = worst(exit, r.exit_code);
            continue;
        };
        // The destination: the file for one input, or the input's relative
        // path under the directory with the emitted container's extension.
        let expected = extensions_for_output(&r.output_format);
        let dest: PathBuf = if batch {
            let relative = input
                .relative
                .clone()
                .or_else(|| path.file_name().map(PathBuf::from))
                .unwrap_or_else(|| PathBuf::from("output"));
            let mut dest = out_path.join(relative);
            if let Some(e) = expected.and_then(|e| e.first()) {
                dest.set_extension(e);
            }
            dest
        } else {
            out_path.to_path_buf()
        };
        if let Some(expected) = expected {
            let ext = dest
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if !expected.contains(&ext.as_str()) {
                eprintln!(
                    "unmark: the run emits {}, so --out must end in .{}; {} does not. Nothing written.",
                    r.output_format,
                    expected.join(" or ."),
                    dest.display()
                );
                r.exit_code = EXIT_USAGE;
                emit(c.output_text, &r);
                exit = worst(exit, EXIT_USAGE);
                continue;
            }
        }
        if same_file(path, &dest) {
            eprintln!(
                "unmark: {} names the input {}. Keeping the input is the backup, so nothing is written.",
                dest.display(),
                path.display()
            );
            r.exit_code = EXIT_USAGE;
            emit(c.output_text, &r);
            exit = worst(exit, EXIT_USAGE);
            continue;
        }
        if written.iter().any(|w| w == &dest) {
            eprintln!(
                "unmark: {} was already written for another input in this run. Nothing written for {}.",
                dest.display(),
                path.display()
            );
            r.exit_code = EXIT_USAGE;
            emit(c.output_text, &r);
            exit = worst(exit, EXIT_USAGE);
            continue;
        }
        if dest.exists() && !c.overwrite {
            eprintln!(
                "unmark: {} exists. Pass --overwrite to replace it.",
                dest.display()
            );
            r.exit_code = EXIT_USAGE;
            emit(c.output_text, &r);
            exit = worst(exit, EXIT_USAGE);
            continue;
        }
        // A no-op in a batch writes nothing; a single --out asks for a
        // file and gets one.
        if r.no_op && batch {
            eprintln!(
                "unmark: {}: no-op, the output equals the input, nothing written",
                path.display()
            );
            emit(c.output_text, &r);
            exit = worst(exit, r.exit_code);
            continue;
        }
        if let Some(parent) = dest.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!("unmark: {}: {e}", parent.display());
                    exit = worst(exit, EXIT_INSTRUMENTATION);
                    continue;
                }
            }
        }
        if let Err(e) = write_atomic(&dest, &out_bytes) {
            eprintln!("unmark: {}: {e}. Nothing written.", dest.display());
            r.error = Some(format!("write failed: {e}"));
            r.exit_code = EXIT_INSTRUMENTATION;
            emit(c.output_text, &r);
            exit = worst(exit, EXIT_INSTRUMENTATION);
            continue;
        }
        r.output = Some(dest.display().to_string());
        written.push(dest.clone());
        emit(c.output_text, &r);
        eprintln!(
            "unmark: wrote {} ({} bytes)",
            dest.display(),
            out_bytes.len()
        );
        exit = worst(exit, r.exit_code);
    }
    Ok(exit)
}

fn cmd_verify(mut parser: lexopt::Parser) -> Result<i32, lexopt::Error> {
    let c = parse_common(&mut parser)?;
    let Some(report_path) = c.report_path else {
        eprintln!("unmark: verify requires --report FILE");
        return Ok(EXIT_USAGE);
    };
    let report_text = match std::fs::read_to_string(&report_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("unmark: {report_path}: {e}");
            return Ok(EXIT_USAGE);
        }
    };
    if c.paths.len() != 1 {
        eprintln!("unmark: verify takes one output path");
        return Ok(EXIT_USAGE);
    }
    let input = match read_input(Path::new(&c.paths[0]), c.max_bytes) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("unmark: {e}");
            return Ok(EXIT_UNSUPPORTED);
        }
    };
    let pkg = match load_policy() {
        Ok(p) => p,
        Err(code) => return Ok(code),
    };
    match unmark::verify(&input, &report_text, &pkg) {
        VerifyOutcome::Verified => {
            println!("{{\"verified\":true}}");
            Ok(EXIT_OK)
        }
        VerifyOutcome::Mismatch(problems) => {
            for p in &problems {
                eprintln!("unmark: verify: {p}");
            }
            println!("{{\"verified\":false}}");
            Ok(report::EXIT_MARK_REMAINS)
        }
    }
}

fn cmd_policy(mut parser: lexopt::Parser) -> Result<i32, lexopt::Error> {
    let mut sub: Option<String> = None;
    let mut out: Option<String> = None;
    while let Some(arg) = parser.next()? {
        match arg {
            Long("out") => out = Some(parser.value()?.string()?),
            Long("help") | Short('h') => {
                println!("{}", usage());
                return Ok(EXIT_OK);
            }
            Value(v) if sub.is_none() => sub = Some(v.string()?),
            arg => return Err(arg.unexpected()),
        }
    }
    let pkg = match load_policy() {
        Ok(p) => p,
        Err(code) => return Ok(code),
    };
    match sub.as_deref() {
        Some("digest") => {
            println!("{}", pkg.digest);
            Ok(EXIT_OK)
        }
        Some("show") => {
            let json = serde_json::json!({
                "version": pkg.version,
                "digest": pkg.digest,
                "supported_containers": pkg.supported_containers,
                "transforms": pkg.transforms.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
                "held": pkg.held.iter().map(|h| h.id.clone()).collect::<Vec<_>>(),
            });
            println!("{json}");
            Ok(EXIT_OK)
        }
        Some("snapshot") => {
            let snapshot = skill::generate(&pkg);
            match out {
                Some(p) => match std::fs::write(&p, snapshot) {
                    Ok(()) => Ok(EXIT_OK),
                    Err(e) => {
                        eprintln!("unmark: {p}: {e}");
                        Ok(EXIT_USAGE)
                    }
                },
                None => {
                    let mut o = std::io::stdout().lock();
                    let _ = o.write_all(snapshot.as_bytes());
                    Ok(EXIT_OK)
                }
            }
        }
        _ => {
            eprintln!("unmark: policy expects digest, show, or snapshot");
            Ok(EXIT_USAGE)
        }
    }
}
