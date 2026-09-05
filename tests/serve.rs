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

/// The arm a command is run in. The repo these tests build has no vectors, so the fused arm
/// never opens the 448 MB model here — what it changes is the flag the handshake is decided on.
const LEXICAL: &[&str] = &["--no-dense"];
const FUSED: &[&str] = &[];

fn ask_in(arm: &[&str], dir: &std::path::Path, extra: &[&str], words: &[&str]) -> (String, String) {
    let out = repograph().args(arm).arg("--repo").arg(dir).arg("ask").args(extra).args(words).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    (String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap())
}

fn ask(dir: &std::path::Path, extra: &[&str], words: &[&str]) -> (String, String) {
    ask_in(LEXICAL, dir, extra, words)
}

/// The same question asked by a chosen binary rather than the one Cargo built, so a test can
/// replace the file under a running server and still ask from the path it replaced.
fn ask_from(bin: &std::path::Path, dir: &std::path::Path, words: &[&str]) -> (String, String) {
    let out = Command::new(bin).args(LEXICAL).arg("--repo").arg(dir).arg("ask").args(words).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    (String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap())
}

/// The two numbers the handshake's build stamp is made of, read the way `walk::stamp_of` reads
/// them. The crate is a binary with no library, so a test computes them rather than calling it.
fn stamp_of(path: &std::path::Path) -> (u64, u64) {
    let m = std::fs::metadata(path).unwrap();
    let ns = m.modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    (u64::try_from(ns).unwrap(), m.len())
}

fn serve_in(arm: &[&str], dir: &std::path::Path, extra: &[&str]) -> std::process::Child {
    repograph().args(arm).arg("--repo").arg(dir).arg("serve").args(extra)
        .stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap()
}

fn serve(dir: &std::path::Path, extra: &[&str]) -> std::process::Child {
    serve_in(LEXICAL, dir, extra)
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
        assert!(err.contains("serve: answered by the resident process"), "the client says it answered through the socket: {err}");
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

// The two tests below are the pairing the socket gate could not see for as long as every test
// passed `--no-dense` on both sides: a server's arm and a client's are two facts, and only one
// of the four pairings diverges.
#[test]
fn a_fused_question_is_refused_by_a_server_that_answers_lexical_only() {
    let dir = repo_with_docs();
    let want = ask_in(FUSED, dir.path(), &["--no-serve"], &["штраф"]).0;
    let mut server = serve(dir.path(), &["--every", "3600", "--idle", "60"]);
    wait_for_socket(dir.path());
    // Asked until the refusal comes rather than once: a socket file says a `serve` bound, and
    // only a reply says it is listening. The line asserted on has one producer, the handshake,
    // so a server that is merely not up yet cannot pass this by falling back for another reason.
    let start = Instant::now();
    let (got, err) = loop {
        let (got, err) = ask_in(FUSED, dir.path(), &[], &["штраф"]);
        if err.contains("serve: the resident process answers lexical-only") { break (got, err); }
        assert!(start.elapsed() < Duration::from_secs(20), "a fused question was answered lexical-only and told nothing: {err}");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(!err.contains("answered by the resident process"), "{err}");
    assert_eq!(got, want, "the client answered it here, as it would have with no server at all");
    server.kill().unwrap();
    let _ = server.wait();
}

#[test]
fn a_lexical_question_is_still_answered_by_a_server_holding_the_dense_arm() {
    let dir = repo_with_docs();
    let want = ask(dir.path(), &["--no-serve"], &["штраф"]).0;
    let mut server = serve_in(FUSED, dir.path(), &["--every", "3600", "--idle", "60"]);
    let (through, _) = ask_until_resident(dir.path(), &["штраф"]);
    assert_eq!(through, want, "a server that opened the model narrows to the arm it was asked in");
    server.kill().unwrap();
    let _ = server.wait();
}

// Every development build of this repository answers `0.4.0`, so the version alone would let
// yesterday's `serve` answer today's question — under `--idle 86400`, for a day.
#[test]
fn a_reply_from_another_build_of_this_version_is_ignored() {
    let dir = repo_with_docs();
    let sock = dir.path().join(".repograph/serve.sock");
    let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
    let fake = std::thread::spawn(move || {
        use std::io::{BufRead, Write};
        let (mut s, _) = listener.accept().unwrap();
        let mut line = String::new();
        std::io::BufReader::new(s.try_clone().unwrap()).read_line(&mut line).unwrap();
        // This version, and a stamp no executable on this machine carries.
        writeln!(s, r#"{{"v":"{}","build":{{"mtime_ns":1,"len":1}},"no_dense":false,"stdout":"WRONG\n","stderr":[]}}"#, env!("CARGO_PKG_VERSION")).unwrap();
    });
    let (out, err) = ask(dir.path(), &[], &["штраф"]);
    assert!(out.contains("FR-PAY-1") && !out.contains("WRONG"), "{out}");
    assert!(err.contains("serve: the resident process is another build"), "{err}");
    fake.join().unwrap();
}

/// `current_exe` names a path, and the file at a server's own path is the one a rebuild replaces.
/// A server that stats it per request reports the stamp of the binary the client is asking from,
/// the two agree, and yesterday's code answers today's question — the case the build stamp exists
/// to refuse. Only a stamp taken once, at start, describes the build a process is running.
#[test]
fn a_binary_replaced_under_a_live_server_is_another_build_and_the_client_answers_here() {
    let dir = repo_with_docs();
    let bin = tempfile::tempdir().unwrap();
    let path = bin.path().join("repograph");
    std::fs::copy(env!("CARGO_BIN_EXE_repograph"), &path).unwrap();
    let started_as = stamp_of(&path);
    let want = ask(dir.path(), &["--no-serve"], &["штраф"]).0;
    let mut server = Command::new(&path).args(LEXICAL).arg("--repo").arg(dir.path())
        .args(["serve", "--every", "3600", "--idle", "60"])
        .stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    // The one build on both ends answers over the socket, which is what makes the run below a
    // test of the replacement rather than of a server that was never reachable.
    let start = Instant::now();
    loop {
        let (_, err) = ask_from(&path, dir.path(), &["штраф"]);
        if err.contains("serve: answered by the resident process") { break; }
        assert!(start.elapsed() < Duration::from_secs(20), "serve never answered over the socket: {err}");
        std::thread::sleep(Duration::from_millis(50));
    }
    // A rebuild in place, as far as a stat can tell: the path now holds a file the running server
    // did not start from. Renamed rather than written over — the running one keeps its inode.
    let replacement = bin.path().join("replacement");
    std::fs::copy(env!("CARGO_BIN_EXE_repograph"), &replacement).unwrap();
    // `fs::copy` carries the source's mtime over on macOS, so the copy would stamp as the file it
    // came from. A build writes its output now, and that is the stamp under test.
    std::fs::File::options().write(true).open(&replacement).unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(std::time::SystemTime::now())).unwrap();
    std::fs::rename(&replacement, &path).unwrap();
    assert_ne!(started_as, stamp_of(&path), "the file at that path stamps differently now");
    let (out, err) = ask_from(&path, dir.path(), &["штраф"]);
    assert!(err.contains("serve: the resident process is another build"),
        "the running server is not the build at its path, and the client says so: {err}");
    assert!(!err.contains("answered by the resident process"), "{err}");
    assert_eq!(out, want, "and the question was answered here instead");
    server.kill().unwrap();
    let _ = server.wait();
}
