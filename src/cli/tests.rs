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
fn publication_handler_reports_usage_without_required_paths() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = handle_publication(&[], &mut stdout, &mut stderr);

    assert_eq!(code, ExitClass::Usage.code());
    let stderr = String::from_utf8(stderr).unwrap();
    assert!(stderr.contains("--verify-root is required"));
    assert!(stderr.contains("allie publication --verify-root"));
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
