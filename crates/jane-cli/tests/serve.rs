//! Three processes on this machine (PORT.md P8): `jane serve` hosting headless and two `jane join`
//! guests played by bot models, over TCP on localhost. The host's hash checks all agree and both
//! guests end on the host's hash. Real time (about 15 s), so ignored by default:
//! `cargo test -p jane-cli --test serve -- --ignored`.

use std::process::{Command, Stdio};

fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

#[test]
#[ignore = "real time: about 15 s"]
fn serve_and_two_headless_guests_hold_one_hash() {
    let exe = env!("CARGO_BIN_EXE_jane");
    let port = free_port().to_string();
    let host = Command::new(exe)
        .args(["serve", "--seed", "3", "--port", &port, "--ticks", "900", "--every", "2"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("jane serve starts");
    let addr = format!("127.0.0.1:{port}");
    let guests: Vec<_> = [("reader", "501"), ("rusher", "502")]
        .into_iter()
        .map(|(model, token)| {
            Command::new(exe)
                .args(["join", &addr, "--model", model, "--token", token, "--every", "2"])
                .stdout(Stdio::piped())
                .spawn()
                .expect("jane join starts")
        })
        .collect();
    let host = host.wait_with_output().unwrap();
    let host_out = String::from_utf8_lossy(&host.stdout).to_string();
    println!("{host_out}");
    assert!(host.status.success());
    let mut finals = Vec::new();
    for g in guests {
        let out = g.wait_with_output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        println!("{text}");
        assert!(out.status.success());
        let last = text.lines().find(|l| l.contains("frames stepped")).expect("a final line").to_owned();
        finals.push(last.rsplit(' ').next().unwrap().to_owned());
    }
    let checks = host_out.lines().find(|l| l.contains("hash checks with guests")).expect("the host's tally");
    let agreed: u64 = checks.rsplit("guests: ").next().unwrap().split(' ').next().unwrap().parse().unwrap();
    assert!(checks.contains(", 0 differed"), "{checks}");
    assert!(agreed >= 16, "{checks}");
    assert_eq!(finals[0], finals[1], "both guests end on one hash");
    assert!(host_out.contains(&format!("hash {}", finals[0])), "the host's hash at that frame too: {}", finals[0]);
}
