//! Thin CLI: parse arguments, call the library, map exits. stdout carries the
//! machine-readable JSON result. Diagnostics go to stderr. `clean` writes the
//! cleaned asset to --out and only when the run reached exit 0.

use lexopt::prelude::*;
use std::io::{Read, Write};
use std::process::ExitCode;
use unmark::report::{self, EXIT_INSTRUMENTATION, EXIT_OK, EXIT_UNSUPPORTED, EXIT_USAGE};
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
                "inspect" => cmd_report(parser, false),
                "plan" => cmd_report(parser, true),
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
     unmark inspect --profile <P> [--output json|text] [--max-bytes N] [PATH | -]\n  \
     unmark plan    --profile <P> [--opt-in MC0x]... [--output json|text] [PATH | -]\n  \
     unmark clean   --profile <P> --out FILE --i-generated-this\n                 \
     [--acknowledge-residual] [--force-provenance-strip] [--overwrite]\n                 \
     [--opt-in MC0x]... [--output json|text] [PATH | -]\n  \
     unmark verify  --report FILE [PATH | -]\n  \
     unmark policy  digest | show | snapshot [--out FILE]\n  \
     unmark --version | -V | --help | -h\n\
     \n\
     profiles (--profile, required, no default):\n  \
     image-metadata, audio-metadata (needs the audio feature), repo-files\n\
     \n\
     exit codes:\n  \
     0   plan applied, every confirmable mark removed and re-proven, residuals acknowledged\n  \
     2   usage error, including a missing ownership assertion\n  \
     10  a confirmable mark is still present in the output\n  \
     20  an unacknowledged residual\n  \
     30  instrumentation error, fail closed\n  \
     40  unsupported input, or the guardrail refused the asset, fail closed"
}

struct Common {
    profile: Option<String>,
    output_text: bool,
    max_bytes: usize,
    path: Option<String>,
    opt_in: Vec<String>,
}

fn read_input(path: Option<&str>, max: usize) -> Result<Vec<u8>, String> {
    let bytes = match path {
        None | Some("-") => {
            let mut buf = Vec::new();
            std::io::stdin()
                .read_to_end(&mut buf)
                .map_err(|e| format!("stdin read: {e}"))?;
            buf
        }
        Some(p) => std::fs::read(p).map_err(|e| format!("{p}: {e}"))?,
    };
    if bytes.len() > max {
        return Err(format!(
            "input is {} bytes, over the {max}-byte limit; raise --max-bytes to process it",
            bytes.len()
        ));
    }
    Ok(bytes)
}

fn set_path(path: &mut Option<String>, value: String) -> Result<(), i32> {
    if path.is_some() {
        eprintln!("unmark: one asset per invocation. Batch mode is not offered for cleaning.");
        return Err(EXIT_USAGE);
    }
    *path = Some(value);
    Ok(())
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

fn cmd_report(mut parser: lexopt::Parser, is_plan: bool) -> Result<i32, lexopt::Error> {
    let c = parse_common(&mut parser)?;
    let Some(profile) = c.profile else {
        eprintln!("unmark: --profile is required (no default)");
        return Ok(EXIT_USAGE);
    };
    let input = match read_input(c.path.as_deref(), c.max_bytes) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("unmark: {e}");
            return Ok(EXIT_UNSUPPORTED);
        }
    };
    let pkg = match policy::load() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("unmark: policy load failed: {e}");
            return Ok(EXIT_INSTRUMENTATION);
        }
    };
    let opts = Options {
        opt_in: c.opt_in,
        ..Options::default()
    };
    let result = if is_plan {
        plan(&input, &profile, &opts, &pkg)
    } else {
        inspect(&input, &profile, &pkg)
    };
    match result {
        Ok(r) => {
            emit(c.output_text, &r);
            Ok(r.exit_code)
        }
        Err(e) => Ok(report_error(e)),
    }
}

fn cmd_clean(mut parser: lexopt::Parser) -> Result<i32, lexopt::Error> {
    let mut profile: Option<String> = None;
    let mut output_text = false;
    let mut max_bytes = DEFAULT_MAX_BYTES;
    let mut path: Option<String> = None;
    let mut out: Option<String> = None;
    let mut overwrite = false;
    let mut i_generated_this = false;
    let mut acknowledge_residual = false;
    let mut force_provenance_strip = false;
    let mut opt_in: Vec<String> = Vec::new();

    while let Some(arg) = parser.next()? {
        match arg {
            Long("profile") => profile = Some(parser.value()?.string()?),
            Long("out") => out = Some(parser.value()?.string()?),
            Long("overwrite") => overwrite = true,
            Long("i-generated-this") => i_generated_this = true,
            Long("acknowledge-residual") => acknowledge_residual = true,
            Long("force-provenance-strip") => force_provenance_strip = true,
            Long("opt-in") => opt_in.push(parser.value()?.string()?),
            Long("output") => output_text = matches!(parser.value()?.string()?.as_str(), "text"),
            Long("max-bytes") => max_bytes = parser.value()?.parse()?,
            Long("help") | Short('h') => {
                println!("{}", usage());
                return Ok(EXIT_OK);
            }
            Value(v) => {
                if let Err(code) = set_path(&mut path, v.string()?) {
                    return Ok(code);
                }
            }
            arg => return Err(arg.unexpected()),
        }
    }

    let Some(profile) = profile else {
        eprintln!("unmark: --profile is required (no default)");
        return Ok(EXIT_USAGE);
    };
    let Some(out) = out else {
        eprintln!("unmark: clean requires --out FILE. Keeping the input is the backup, so there is no in-place mode.");
        return Ok(EXIT_USAGE);
    };
    if std::path::Path::new(&out).exists() && !overwrite {
        eprintln!("unmark: {out} exists. Pass --overwrite to replace it.");
        return Ok(EXIT_USAGE);
    }

    let input = match read_input(path.as_deref(), max_bytes) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("unmark: {e}");
            return Ok(EXIT_UNSUPPORTED);
        }
    };
    let pkg = match policy::load() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("unmark: policy load failed: {e}");
            return Ok(EXIT_INSTRUMENTATION);
        }
    };
    let opts = Options {
        i_generated_this,
        acknowledge_residual,
        force_provenance_strip,
        opt_in,
    };
    match clean(&input, &profile, &opts, &pkg) {
        Ok(outcome) => {
            emit(output_text, &outcome.report);
            if let Some(bytes) = outcome.output {
                if let Err(e) = std::fs::write(&out, &bytes) {
                    eprintln!("unmark: {out}: {e}");
                    return Ok(EXIT_INSTRUMENTATION);
                }
                eprintln!("unmark: wrote {} ({} bytes)", out, bytes.len());
            }
            Ok(outcome.report.exit_code)
        }
        Err(e) => Ok(report_error(e)),
    }
}

fn cmd_verify(mut parser: lexopt::Parser) -> Result<i32, lexopt::Error> {
    let mut report_path: Option<String> = None;
    let mut path: Option<String> = None;
    while let Some(arg) = parser.next()? {
        match arg {
            Long("report") => report_path = Some(parser.value()?.string()?),
            Long("help") | Short('h') => {
                println!("{}", usage());
                return Ok(EXIT_OK);
            }
            Value(v) => {
                if let Err(code) = set_path(&mut path, v.string()?) {
                    return Ok(code);
                }
            }
            arg => return Err(arg.unexpected()),
        }
    }
    let Some(report_path) = report_path else {
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
    let input = match read_input(path.as_deref(), DEFAULT_MAX_BYTES) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("unmark: {e}");
            return Ok(EXIT_UNSUPPORTED);
        }
    };
    let pkg = match policy::load() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("unmark: policy load failed: {e}");
            return Ok(EXIT_INSTRUMENTATION);
        }
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
    let pkg = match policy::load() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("unmark: policy load failed: {e}");
            return Ok(EXIT_INSTRUMENTATION);
        }
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
                "profiles": pkg.profiles.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
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

fn parse_common(parser: &mut lexopt::Parser) -> Result<Common, lexopt::Error> {
    let mut c = Common {
        profile: None,
        output_text: false,
        max_bytes: DEFAULT_MAX_BYTES,
        path: None,
        opt_in: Vec::new(),
    };
    while let Some(arg) = parser.next()? {
        match arg {
            Long("profile") => c.profile = Some(parser.value()?.string()?),
            Long("output") => c.output_text = matches!(parser.value()?.string()?.as_str(), "text"),
            Long("max-bytes") => c.max_bytes = parser.value()?.parse()?,
            Long("opt-in") => c.opt_in.push(parser.value()?.string()?),
            Long("help") | Short('h') => {
                println!("{}", usage());
                std::process::exit(EXIT_OK);
            }
            Value(v) => {
                if set_path(&mut c.path, v.string()?).is_err() {
                    std::process::exit(EXIT_USAGE);
                }
            }
            arg => return Err(arg.unexpected()),
        }
    }
    Ok(c)
}

fn report_error(e: UnmarkError) -> i32 {
    match e {
        UnmarkError::Usage(m) => {
            eprintln!("unmark: {m}");
            EXIT_USAGE
        }
        UnmarkError::Unsupported(m) => {
            eprintln!("unmark: unsupported_input: {m}");
            EXIT_UNSUPPORTED
        }
        UnmarkError::Malformed(m) => {
            eprintln!("unmark: malformed: {m}");
            EXIT_UNSUPPORTED
        }
        UnmarkError::Instrumentation(m) => {
            eprintln!("unmark: instrumentation_error: {m}");
            EXIT_INSTRUMENTATION
        }
    }
}
