use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn repograph() -> Command { Command::new(env!("CARGO_BIN_EXE_repograph")) }

fn repo_with_docs() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/pay.md"), "**FR-PAY-1 · MUST · Штраф за отмену**\n\nШтраф списывается сам (INV-1).\n\n**INV-1 · MUST · Деньги не сгорают**\n\nОтмена не сжигает деньги.\n").unwrap();
    std::fs::write(dir.path().join("docs/cal.md"), "**FR-CAL-1 · MUST · Перенос визита**\n\nПеренос не считается отменой.\n").unwrap();
    std::fs::write(dir.path().join("repograph.toml"), "id_families = [\"FR-PAY\", \"FR-CAL\", \"INV\"]\n").unwrap();
    let out = repograph().args(["--no-dense", "--repo"]).arg(dir.path()).arg("build").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    dir
}

fn ask(dir: &std::path::Path, extra: &[&str], words: &[&str]) -> (String, String) {
    let out = repograph().args(["--no-dense", "--repo"]).arg(dir).arg("ask").args(extra).args(words).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    (String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap())
}

fn wait_for_socket(dir: &std::path::Path) {
    let sock = dir.join(".repograph/serve.sock");
    let start = Instant::now();
    while !sock.exists() {
        assert!(start.elapsed() < Duration::from_secs(20), "serve never opened its socket");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn the_socket_answers_the_bytes_the_process_answers_and_sees_an_edit() {
    let dir = repo_with_docs();
    let questions = [vec!["штраф"], vec!["перенос", "визита"], vec!["FR-CAL-1"], vec!["ничего"]];
    let direct: Vec<_> = questions.iter().map(|q| ask(dir.path(), &["--no-serve"], q).0).collect();
    let mut server = repograph().args(["--no-dense", "--repo"]).arg(dir.path()).args(["serve", "--every", "1", "--idle", "60"]).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    wait_for_socket(dir.path());
    for (q, want) in questions.iter().zip(&direct) {
        let (got, err) = ask(dir.path(), &[], q);
        assert_eq!(&got, want, "question {q:?}");
        assert!(err.contains("serve:"), "the client says it answered through the socket: {err}");
    }
    let json_direct = ask(dir.path(), &["--no-serve", "--json"], &["штраф"]).0;
    assert_eq!(ask(dir.path(), &["--json"], &["штраф"]).0, json_direct);
    std::fs::write(dir.path().join("docs/new.md"), "**FR-PAY-2 · MUST · Возврат аванса**\n\nАванс возвращается при отмене салоном.\n").unwrap();
    let (after, _) = ask(dir.path(), &[], &["возврат", "аванса"]);
    assert!(after.contains("FR-PAY-2"), "the server refreshed before answering: {after}");
    server.kill().unwrap();
    let _ = server.wait();
}

#[test]
fn a_socket_nobody_listens_on_is_removed_and_the_question_answered_here() {
    let dir = repo_with_docs();
    let sock = dir.path().join(".repograph/serve.sock");
    std::os::unix::net::UnixListener::bind(&sock).unwrap();
    // The listener is dropped at once: the file stays, nothing accepts.
    let (out, err) = ask(dir.path(), &[], &["штраф"]);
    assert!(out.contains("FR-PAY-1"));
    assert!(!err.contains("serve:"), "{err}");
    assert!(!sock.exists(), "a dead socket file is unlinked");
}

#[test]
fn a_reply_from_another_version_is_ignored() {
    let dir = repo_with_docs();
    let sock = dir.path().join(".repograph/serve.sock");
    let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
    let fake = std::thread::spawn(move || {
        use std::io::{BufRead, Write};
        let (mut s, _) = listener.accept().unwrap();
        let mut line = String::new();
        std::io::BufReader::new(s.try_clone().unwrap()).read_line(&mut line).unwrap();
        writeln!(s, r#"{{"v":"0.0.0","stdout":"WRONG\n","stderr":[]}}"#).unwrap();
    });
    let (out, _) = ask(dir.path(), &[], &["штраф"]);
    assert!(out.contains("FR-PAY-1") && !out.contains("WRONG"), "{out}");
    fake.join().unwrap();
}
