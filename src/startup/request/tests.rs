use super::*;

fn parse(args: &[&str]) -> anyhow::Result<Request> {
    Request::parse(
        args.iter().map(OsString::from).collect(),
        std::env::temp_dir(),
    )
}

#[test]
fn external_launches_use_the_callers_directory() {
    assert_eq!(parse(&[]).unwrap().directory(), std::env::temp_dir());
    assert!(!parse(&[]).unwrap().new_window);
    assert!(parse(&["--new-window"]).unwrap().new_window);
}

#[test]
fn directory_aliases_and_relative_paths_agree() {
    let cwd = std::env::current_dir().unwrap();
    for flag in ["--working-directory", "--workdir", "--directory"] {
        let args = vec![OsString::from(flag), cwd.as_os_str().to_owned()];
        assert_eq!(
            Request::parse(args, std::env::temp_dir())
                .unwrap()
                .directory(),
            cwd
        );
        let arg = OsString::from(format!("{flag}={}", cwd.display()));
        assert_eq!(
            Request::parse(vec![arg], std::env::temp_dir())
                .unwrap()
                .directory(),
            cwd
        );
    }
    let request = Request::parse(vec!["--workdir".into(), ".".into()], cwd.clone()).unwrap();
    assert_eq!(request.directory(), cwd.join("."));
}

#[test]
fn paths_survive_the_wire_without_shell_quoting_or_utf8_conversion() {
    use std::os::unix::ffi::OsStringExt;
    let mut request = parse(&[]).unwrap();
    request.directory = b"/tmp/\xe4\xb8\xad\xe6\x96\x87 ' \" $() \xff".to_vec();
    let bytes = serde_json::to_vec(&request).unwrap();
    let decoded: Request = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        decoded.directory().into_os_string().into_vec(),
        request.directory
    );
}

#[test]
fn commands_keep_arguments_including_flags_literal() {
    let request = parse(&[
        "--new-window",
        "-e",
        "printf",
        "%s",
        "hello world",
        "--new-window",
    ])
    .unwrap();
    assert_eq!(
        request.command,
        ["printf", "%s", "hello world", "--new-window"]
    );
    assert!(request.new_window);
    assert_eq!(
        parse(&["--command", "printf hi | cat"]).unwrap().command,
        ["printf hi | cat"]
    );
}

#[test]
fn invalid_launches_are_rejected_before_reaching_the_window() {
    for args in [
        &["--workdir"][..],
        &["--workdir="],
        &["-e"],
        &["--unknown"],
        &["--workdir", "/definitely-not-an-ely-directory"],
    ] {
        assert!(parse(args).is_err(), "{args:?}");
    }
    let mut request = parse(&[]).unwrap();
    request.command = vec!["bad\0command".into()];
    assert!(request.validate().is_err());
}
