use crate::{
    ExitClass, NEXT_STEP, PRODUCT_LINE, parse_discovery_options, parse_doctor_options,
    parse_map_options, parse_promote_flow_options, parse_release_options, parse_report_options,
    parse_review_options, parse_run_options, run_compliance_report, run_discovery, run_map,
    run_promote_flow, run_release, run_review, run_v0,
};
use crate::{consumer, workbench, worker_runtime};
use std::fmt::Display;
use std::io::{self, Write};

pub(crate) fn run_cli(args: impl IntoIterator<Item = String>) -> i32 {
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    run_cli_with_io(args, &mut stdout, &mut stderr)
}

pub(crate) fn run_cli_with_io(
    args: impl IntoIterator<Item = String>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let args = args.into_iter().collect::<Vec<_>>();

    if args.is_empty() {
        let _ = writeln!(stdout, "{PRODUCT_LINE}");
        let _ = writeln!(stdout, "{NEXT_STEP}");
        let _ = writeln!(
            stdout,
            "Run: allie run --manifest examples/login-flow.yml --out .allie/runs/latest"
        );
        return ExitClass::Success.code();
    }

    if matches!(args.first().map(String::as_str), Some("-h" | "--help")) {
        print_usage(stdout);
        return ExitClass::Success.code();
    }

    match args.first().map(String::as_str) {
        Some("init") => handle_init(&args[1..], stdout, stderr),
        Some("verify") => handle_verify(&args[1..], stdout, stderr),
        Some("doctor") => handle_doctor(&args[1..], stdout, stderr),
        Some("run") => handle_run(&args[1..], stdout, stderr),
        Some("discover") => handle_discover(&args[1..], stdout, stderr),
        Some("promote-flow") => handle_promote_flow(&args[1..], stdout, stderr),
        Some("map") => handle_map(&args[1..], stdout, stderr),
        Some("report") => handle_report(&args[1..], stdout, stderr),
        Some("workbench") => handle_workbench(&args[1..], stdout, stderr),
        Some("review") => handle_review(&args[1..], stdout, stderr),
        Some("release") => handle_release(&args[1..], stdout, stderr),
        _ => {
            let _ = writeln!(stderr, "allie: unknown command");
            print_usage(stderr);
            ExitClass::Usage.code()
        }
    }
}

fn handle_doctor(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match parse_doctor_options(args) {
        Ok(options) => {
            let receipt = worker_runtime::run_doctor(worker_runtime::DoctorOptions {
                manifest_path: options.manifest_path,
                out_dir: options.out_dir,
            });
            let _ = writeln!(stdout, "Allie doctor status: {}", receipt.status);
            for check in receipt.checks {
                let _ = writeln!(
                    stdout,
                    "{}: {} - {}",
                    check.name, check.status, check.detail
                );
                if let Some(fix) = check.fix {
                    let _ = writeln!(stdout, "  fix: {fix}");
                }
            }
            receipt.exit_class.code()
        }
        Err(error) => usage_error(error, stderr),
    }
}

fn handle_init(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match consumer::parse_init_options(args) {
        Ok(options) => match consumer::run_init(options) {
            Ok(receipt) => {
                let _ = writeln!(
                    stdout,
                    "Allie manifest: {}",
                    receipt.manifest_path.display()
                );
                if let Some(note) = &receipt.model_note {
                    let _ = writeln!(stdout, "{note}");
                }
                let _ = writeln!(stdout, "Setup checklist:");
                for step in receipt.setup_steps {
                    let _ = writeln!(stdout, "  - {step}");
                }
                let _ = writeln!(stdout, "Next: {}", receipt.next_command);
                ExitClass::Success.code()
            }
            Err(error) => infra_error(error, stderr),
        },
        Err(error) => usage_error(error, stderr),
    }
}

fn handle_verify(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match consumer::parse_verify_options(args) {
        Ok(options) => match consumer::run_verify(options) {
            Ok(receipt) => {
                let _ = writeln!(stdout, "Allie verification status: {}", receipt.status);
                let _ = writeln!(
                    stdout,
                    "Summary JSON: {}",
                    receipt.summary_json_path.display()
                );
                let _ = writeln!(
                    stdout,
                    "Summary Markdown: {}",
                    receipt.summary_markdown_path.display()
                );
                let _ = writeln!(
                    stdout,
                    "Report JSON: {}",
                    receipt.report_json_path.display()
                );
                let _ = writeln!(
                    stdout,
                    "Report HTML: {}",
                    receipt.report_html_path.display()
                );
                let _ = writeln!(stdout, "JUnit: {}", receipt.junit_path.display());
                let _ = writeln!(stdout, "SARIF: {}", receipt.sarif_path.display());
                let _ = writeln!(
                    stdout,
                    "Release summary: {}",
                    receipt.release_summary_path.display()
                );
                let _ = writeln!(
                    stdout,
                    "Product map: {}",
                    receipt.product_map_path.display()
                );
                let _ = writeln!(stdout, "Evidence: {}", receipt.evidence_path.display());
                receipt.exit_class.code()
            }
            Err(error) => infra_error(error, stderr),
        },
        Err(error) => usage_error(error, stderr),
    }
}

fn handle_run(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match parse_run_options(args) {
        Ok(options) => match run_v0(options) {
            Ok(receipt) => {
                let _ = writeln!(stdout, "Allie evidence run: {}", receipt.run_id);
                let _ = writeln!(stdout, "Evidence: {}", receipt.evidence_path.display());
                let _ = writeln!(stdout, "Report: {}", receipt.report_path.display());
                let _ = writeln!(stdout, "Status: {}", receipt.exit_class.packet_status());
                receipt.exit_class.code()
            }
            Err(error) => infra_error(error, stderr),
        },
        Err(error) => usage_error(error, stderr),
    }
}

fn handle_discover(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match parse_discovery_options(args) {
        Ok(options) => match run_discovery(options) {
            Ok(receipt) => {
                let _ = writeln!(stdout, "Discovery: {}", receipt.discovery_path.display());
                let _ = writeln!(stdout, "Flow plan: {}", receipt.flow_plan_path.display());
                let _ = writeln!(stdout, "Report: {}", receipt.report_path.display());
                ExitClass::Success.code()
            }
            Err(error) => infra_error(error, stderr),
        },
        Err(error) => usage_error(error, stderr),
    }
}

fn handle_promote_flow(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match parse_promote_flow_options(args) {
        Ok(options) => match run_promote_flow(options) {
            Ok(receipt) => {
                let _ = writeln!(
                    stdout,
                    "Generated manifest: {}",
                    receipt.manifest_path.display()
                );
                ExitClass::Success.code()
            }
            Err(error) => infra_error(error, stderr),
        },
        Err(error) => usage_error(error, stderr),
    }
}

fn handle_map(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match parse_map_options(args) {
        Ok(options) => match run_map(options) {
            Ok(receipt) => {
                let _ = writeln!(stdout, "Product map: {}", receipt.map_path.display());
                let _ = writeln!(stdout, "Surface map: {}", receipt.report_path.display());
                let _ = writeln!(
                    stdout,
                    "Agent receipt: {}",
                    receipt.runner_receipt_path.display()
                );
                let _ = writeln!(
                    stdout,
                    "Generated flow: {}",
                    receipt.flow_manifest_path.display()
                );
                ExitClass::Success.code()
            }
            Err(error) => infra_error(error, stderr),
        },
        Err(error) => usage_error(error, stderr),
    }
}

fn handle_report(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match parse_report_options(args) {
        Ok(options) => match run_compliance_report(options) {
            Ok(receipt) => {
                let _ = writeln!(
                    stdout,
                    "Compliance JSON: {}",
                    receipt.report_json_path.display()
                );
                let _ = writeln!(
                    stdout,
                    "Compliance report: {}",
                    receipt.report_html_path.display()
                );
                let _ = writeln!(stdout, "Summary: {}", receipt.summary_path.display());
                ExitClass::Success.code()
            }
            Err(error) => infra_error(error, stderr),
        },
        Err(error) => usage_error(error, stderr),
    }
}

fn handle_workbench(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match workbench::parse_workbench_command(args) {
        Ok(command) => match workbench::run_workbench(command) {
            Ok(receipt) => {
                let _ = writeln!(stdout, "Workbench job: {}", receipt.job_path.display());
                let _ = writeln!(stdout, "Events: {}", receipt.events_path.display());
                let _ = writeln!(stdout, "Status: {}", receipt.status);
                let _ = writeln!(stdout, "Current step: {}", receipt.current_step);
                let _ = writeln!(stdout, "Resumable: {}", receipt.resumable);
                receipt.exit_class.code()
            }
            Err(error) => infra_error(error, stderr),
        },
        Err(error) => usage_error(error, stderr),
    }
}

fn handle_review(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match parse_review_options(args) {
        Ok(options) => match run_review(options) {
            Ok(receipt) => {
                let _ = writeln!(stdout, "Reviewed packet: {}", receipt.packet_path.display());
                let _ = writeln!(stdout, "Review report: {}", receipt.report_path.display());
                ExitClass::Success.code()
            }
            Err(error) => infra_error(error, stderr),
        },
        Err(error) => usage_error(error, stderr),
    }
}

fn handle_release(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match parse_release_options(args) {
        Ok(options) => match run_release(options) {
            Ok(receipt) => {
                let _ = writeln!(
                    stdout,
                    "Release summary: {}",
                    receipt.summary_path.display()
                );
                let _ = writeln!(stdout, "GitHub check: {}", receipt.check_path.display());
                let _ = writeln!(stdout, "Release report: {}", receipt.report_path.display());
                let _ = writeln!(stdout, "Status: {}", receipt.status);
                receipt.exit_class.code()
            }
            Err(error) => infra_error(error, stderr),
        },
        Err(error) => usage_error(error, stderr),
    }
}

fn infra_error(error: impl Display, stderr: &mut dyn Write) -> i32 {
    let _ = writeln!(stderr, "allie: {error}");
    ExitClass::InfrastructureFailure.code()
}

fn usage_error(error: impl Display, stderr: &mut dyn Write) -> i32 {
    let _ = writeln!(stderr, "allie: {error}");
    print_usage(stderr);
    ExitClass::Usage.code()
}

fn print_usage(writer: &mut dyn Write) {
    let _ = writeln!(
        writer,
        "Usage:\n  allie init [--manifest .allie/manifest.yml] [--app-name <name>] [--base-url <url> | --fixture-dir <dir>] [--force]\n  allie doctor [--manifest .allie/manifest.yml | --no-manifest] [--out .allie/doctor]\n  allie verify [--manifest .allie/manifest.yml] [--out .allie/verify/latest] [--project-root <dir>] [--changed-surface <id>] [--agent local|opencode|omp] [--stale-after-days <days>]\n  allie run --manifest <flow.yml> --out <output-dir> [--project-root <dir>]\n  allie discover --manifest <flow.yml> --out <output-dir>\n  allie promote-flow --discovery <discovery.json> --flow-plan <flow-plan.json> --out <flow.yml>\n  allie map --manifest <flow.yml> --out <output-dir> [--project-root <dir>] [--agent local|opencode|omp]\n  allie report --map <product-map.json> --packet <evidence.json> --out <output-dir>\n  allie workbench start --manifest <flow.yml> --out <job-dir> [--project-root <dir>]\n  allie workbench status --job <job-dir>\n  allie workbench cancel --job <job-dir>\n  allie workbench resume --job <job-dir>\n  allie review --packet <evidence.json> --out <output-dir>\n  allie release --packet <evidence.json> --out <output-dir> [--changed-surface <id>] [--stale-after-days <days>]"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_handler_reports_usage_without_required_paths() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = handle_run(&[], &mut stdout, &mut stderr);

        assert_eq!(code, ExitClass::Usage.code());
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("--manifest is required"));
        assert!(stderr.contains("allie run --manifest"));
    }

    #[test]
    fn release_handler_reports_usage_without_packet() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = handle_release(&[], &mut stdout, &mut stderr);

        assert_eq!(code, ExitClass::Usage.code());
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("--packet is required"));
        assert!(stderr.contains("allie release --packet"));
    }

    #[test]
    fn doctor_handler_reports_usage_for_unexpected_args() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = handle_doctor(&["--wat".to_string()], &mut stdout, &mut stderr);

        assert_eq!(code, ExitClass::Usage.code());
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("unexpected argument: --wat"));
        assert!(stderr.contains("allie doctor"));
    }
}
