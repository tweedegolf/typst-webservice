use super::{AddrOverride, parse_addr_arg, parse_cli_args_from};

#[test]
fn parse_addr_arg_accepts_port_only() {
    let parsed = parse_addr_arg("8085");
    assert!(matches!(parsed, Some(AddrOverride::Port(8085))));
}

#[test]
fn parse_addr_arg_accepts_host_port() {
    let parsed = parse_addr_arg("127.0.0.1:9090");
    assert!(matches!(
        parsed,
        Some(AddrOverride::Full(addr)) if addr == "127.0.0.1:9090"
    ));
}

#[test]
fn parse_addr_arg_rejects_missing_host_or_port() {
    assert!(parse_addr_arg(":8080").is_none());
    assert!(parse_addr_arg("localhost:").is_none());
}

#[test]
fn parse_cli_args_detects_addr_and_assets_any_order() {
    let cli = parse_cli_args_from(vec!["./assets", "0.0.0.0:8081"]);
    assert!(matches!(
        cli.addr,
        Some(AddrOverride::Full(addr)) if addr == "0.0.0.0:8081"
    ));
    assert_eq!(cli.assets_dir.as_deref(), Some("./assets"));

    let cli = parse_cli_args_from(vec!["8081", "./assets"]);
    assert!(matches!(cli.addr, Some(AddrOverride::Port(8081))));
    assert_eq!(cli.assets_dir.as_deref(), Some("./assets"));
}

#[test]
fn parse_cli_args_flags_version_anywhere() {
    let cli = parse_cli_args_from(vec!["./assets", "--version", "127.0.0.1:9999"]);
    assert!(cli.show_version);
    assert!(matches!(
        cli.addr,
        Some(AddrOverride::Full(addr)) if addr == "127.0.0.1:9999"
    ));
    assert_eq!(cli.assets_dir.as_deref(), Some("./assets"));
}

#[test]
fn parse_cli_args_collects_extra_args() {
    let cli = parse_cli_args_from(vec!["127.0.0.1:8082", "./assets", "extra", "more"]);
    assert!(matches!(
        cli.addr,
        Some(AddrOverride::Full(addr)) if addr == "127.0.0.1:8082"
    ));
    assert_eq!(cli.assets_dir.as_deref(), Some("./assets"));
    assert_eq!(cli.extra, vec!["extra".to_string(), "more".to_string()]);
}
