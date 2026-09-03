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
     There are no profiles. Each sniffed container receives one default run that strips every\n\
     mark it can find. A directory PATH processes every supported file in it. --out names a file\n\
     for one input, or a directory for several; the extension must match the emitted container.\n\
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

/// Expand directory arguments into their regular files, sorted, so a batch
/// runs in a fixed order.
fn expand_inputs(paths: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for p in paths {
        if p == "-" {
            out.push(PathBuf::from("-"));
            continue;
        }
        let path = Path::new(p);
        if path.is_dir() {
            let mut files: Vec<PathBuf> = std::fs::read_dir(path)
                .map_err(|e| format!("{p}: {e}"))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|f| f.is_file())
                .collect();
            files.sort();
            out.extend(files);
        } else if path.is_file() {
            out.push(path.to_path_buf());
        } else {
            return Err(format!("{p}: no such file or directory"));
        }
    }
    if out.is_empty() {
        return Err("no input path given".to_string());
    }
    Ok(out)
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

fn cmd_survey(mut parser: lexopt::Parser, is_plan: bool) -> Result<i32, lexopt::Error> {
    let c = parse_common(&mut parser)?;
    let inputs = match expand_inputs(&c.paths) {
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
    let mut exit = EXIT_OK;
    for path in inputs {
        let input = match read_input(&path, c.max_bytes) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("unmark: {e}");
                exit = worst(exit, EXIT_UNSUPPORTED);
                continue;
            }
        };
        let result = if is_plan {
            plan(&input, &c.opts, &pkg)
        } else {
            inspect(&input, &c.opts, &pkg)
        };
        match result {
            Ok(r) => {
                emit(c.output_text, &r);
                exit = worst(exit, r.exit_code);
            }
            Err(e) => exit = worst(exit, report_error(&path, e)),
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

fn cmd_clean(mut parser: lexopt::Parser) -> Result<i32, lexopt::Error> {
    let c = parse_common(&mut parser)?;
    let Some(out) = c.out.clone() else {
        eprintln!("unmark: clean requires --out PATH or --out DIR. Keeping the input is the backup, so there is no in-place mode.");
        return Ok(EXIT_USAGE);
    };
    let inputs = match expand_inputs(&c.paths) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("unmark: {e}");
            return Ok(EXIT_USAGE);
        }
    };
    let out_path = Path::new(&out);
    let batch = inputs.len() > 1 || out_path.is_dir();
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
    for path in inputs {
        let input = match read_input(&path, c.max_bytes) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("unmark: {e}");
                exit = worst(exit, EXIT_UNSUPPORTED);
                continue;
            }
        };
        let outcome = match clean(&input, &c.opts, &pkg) {
            Ok(o) => o,
            Err(e) => {
                exit = worst(exit, report_error(&path, e));
                continue;
            }
        };
        let mut r = outcome.report;
        let Some(bytes) = outcome.output else {
            emit(c.output_text, &r);
            exit = worst(exit, r.exit_code);
            continue;
        };
        // The destination: a file for one input, or the input's stem under
        // the directory with the emitted container's extension.
        let expected = extensions_for_output(&r.output_format);
        let dest: PathBuf = if batch {
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "output".to_string());
            // The emitted container's extension, or the input's own for a
            // text file.
            let ext = expected
                .and_then(|e| e.first())
                .map(|e| e.to_string())
                .or_else(|| path.extension().map(|e| e.to_string_lossy().to_string()));
            match ext {
                Some(e) => out_path.join(format!("{stem}.{e}")),
                None => out_path.join(stem),
            }
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
        if let Err(e) = std::fs::write(&dest, &bytes) {
            eprintln!("unmark: {}: {e}", dest.display());
            exit = worst(exit, EXIT_INSTRUMENTATION);
            continue;
        }
        emit(c.output_text, &r);
        eprintln!("unmark: wrote {} ({} bytes)", dest.display(), bytes.len());
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

fn report_error(path: &Path, e: UnmarkError) -> i32 {
    match e {
        UnmarkError::Usage(m) => {
            eprintln!("unmark: {}: {m}", path.display());
            EXIT_USAGE
        }
        UnmarkError::Unsupported(m) => {
            eprintln!("unmark: {}: unsupported_input: {m}", path.display());
            EXIT_UNSUPPORTED
        }
        UnmarkError::Inspection(m) => {
            eprintln!("unmark: {}: inspection_failed: {m}", path.display());
            EXIT_INSTRUMENTATION
        }
    }
}
