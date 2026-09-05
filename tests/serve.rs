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

fn serve(dir: &std::path::Path, extra: &[&str]) -> std::process::Child {
    repograph().args(["--no-dense", "--repo"]).arg(dir).arg("serve").args(extra)
        .stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap()
}

/// The answer once a server is the one giving it. A socket file that exists says a `serve` has
/// bound; only a reply says it is listening, so this asks until one comes rather than sleeping.
fn ask_until_resident(dir: &std::path::Path, words: &[&str]) -> (String, String) {
    let start = Instant::now();
    loop {
        let (out, err) = ask(dir, &[], words);
        if err.contains("serve: answered by the resident process") { return (out, err); }
        assert!(start.elapsed() < Duration::from_secs(20), "serve never answered over the socket: {err}");
        std::thread::sleep(Duration::from_millis(50));
    }
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
    // Polling every hour, so the edit below can only be seen by the refresh before the answer —
    // the behaviour this test is named for, rather than a background poll that beat it to it.
    let mut server = serve(dir.path(), &["--every", "3600", "--idle", "60"]);
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
fn a_socket_nobody_listens_on_is_answered_here_and_left_for_the_next_serve() {
    let dir = repo_with_docs();
    let sock = dir.path().join(".repograph/serve.sock");
    std::os::unix::net::UnixListener::bind(&sock).unwrap();
    // The listener is dropped at once: the file stays, nothing accepts.
    let (out, err) = ask(dir.path(), &[], &["штраф"]);
    let want = ask(dir.path(), &["--no-serve"], &["штраф"]).0;
    assert_eq!(out, want);
    assert!(out.contains("FR-PAY-1"));
    assert!(!err.contains("serve:"), "{err}");
    // A client that cannot be answered falls back; it does not delete files. A refused connect
    // is also what a live server with a full backlog gives, and unlinking on that guess would
    // strand it. `serve` owns the file, and binds over a dead one.
    assert!(sock.exists(), "the client leaves the socket file alone");
    let mut server = serve(dir.path(), &["--every", "3600", "--idle", "60"]);
    let (through, _) = ask_until_resident(dir.path(), &["штраф"]);
    assert_eq!(through, want);
    server.kill().unwrap();
    let _ = server.wait();
}

#[test]
fn an_edited_config_stops_the_server_rather_than_answering_under_the_old_one() {
    let dir = repo_with_docs();
    let mut server = serve(dir.path(), &["--every", "1", "--idle", "60"]);
    wait_for_socket(dir.path());
    std::fs::write(dir.path().join("repograph.toml"), "id_families = [\"FR-PAY\", \"FR-CAL\", \"INV\", \"NFR\"]\n").unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = server.try_wait().unwrap() {
            assert!(status.success(), "{status}");
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "the server kept answering under the old config");
        std::thread::sleep(Duration::from_millis(50));
    }
    // And the question is answered here, under the file as it now reads.
    assert!(ask(dir.path(), &[], &["штраф"]).0.contains("FR-PAY-1"));
}

#[test]
fn a_stale_socket_answer_is_no_older_than_the_store_on_disk() {
    let dir = repo_with_docs();
    // The one poll this server will do for the whole test happened at start-up: whatever it
    // learns later, it learns because a request made it look, not because a timer did.
    let mut server = serve(dir.path(), &["--every", "3600", "--idle", "60"]);
    wait_for_socket(dir.path());
    assert!(!ask_until_resident(dir.path(), &["возврат", "аванса"]).0.contains("FR-PAY-2"));
    std::fs::write(dir.path().join("docs/new.md"), "**FR-PAY-2 · MUST · Возврат аванса**\n\nАванс возвращается при отмене салоном.\n").unwrap();
    // Another process writes the store — the walk is its, not the server's.
    let want = ask(dir.path(), &["--no-serve"], &["возврат", "аванса"]).0;
    assert!(want.contains("FR-PAY-2"), "{want}");
    let (got, err) = ask(dir.path(), &["--stale"], &["возврат", "аванса"]);
    assert!(err.contains("serve: answered by the resident process"), "{err}");
    assert_eq!(got, ask(dir.path(), &["--no-serve", "--stale"], &["возврат", "аванса"]).0);
    assert!(got.contains("FR-PAY-2"), "a stale answer still reads the store on disk: {got}");
    server.kill().unwrap();
    let _ = server.wait();
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
