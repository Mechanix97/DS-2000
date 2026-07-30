//! Diagnostic: raw handshake against Discord's RPC pipe, bypassing [`discord::ipc::IpcClient`].
//!
//! For a bug report this separates a Discord-side problem from one in our client in a single
//! step: it writes the frames by hand and prints whatever comes back.
//!
//! Run with: `cargo run -p discord --example rawpipe -- <client_id>`
//!
//! Windows only, since it speaks to a named pipe directly. The Unix socket path is left out
//! on purpose — this is a debugging aid for the platform the app actually supports.

#[cfg(not(windows))]
fn main() {
    eprintln!("rawpipe is a Windows-only diagnostic");
}

#[cfg(windows)]
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[cfg(windows)]
use tokio::net::windows::named_pipe::ClientOptions;

/// Replaces an OAuth code with a placeholder, keeping the rest of the frame readable.
#[cfg(windows)]
fn redact_code(frame: &str) -> String {
    let Some(start) = frame.find(r#""code":""#) else {
        return frame.to_owned();
    };
    let value_start = start + r#""code":""#.len();
    match frame[value_start..].find('"') {
        Some(offset) => format!(
            "{}<redacted>{}",
            &frame[..value_start],
            &frame[value_start + offset..]
        ),
        None => frame.to_owned(),
    }
}

#[cfg(windows)]
#[tokio::main]
async fn main() {
    let client_id = std::env::args().nth(1).expect("client id");

    let mut pipe = None;
    for i in 0..10 {
        let name = format!(r"\\?\pipe\discord-ipc-{i}");
        match ClientOptions::new().open(&name) {
            Ok(p) => {
                println!("opened {name}");
                pipe = Some(p);
                break;
            }
            Err(e) => println!(
                "  {name}: {:?} raw_os_error={:?}",
                e.kind(),
                e.raw_os_error()
            ),
        }
    }
    let mut pipe = pipe.expect("no discord pipe");

    let payload = format!(r#"{{"v":1,"client_id":"{client_id}"}}"#);
    let mut frame = Vec::new();
    frame.extend(&0u32.to_le_bytes());
    frame.extend(&(payload.len() as u32).to_le_bytes());
    frame.extend(payload.as_bytes());
    println!("writing handshake ({} bytes): {payload}", frame.len());
    pipe.write_all(&frame).await.expect("write");
    println!("written, waiting for a reply...");

    // Read READY, then ask for authorisation and see what comes back.
    let mut sent_authorize = false;

    for n in 1..=4 {
        if n == 2 && !sent_authorize {
            sent_authorize = true;
            let auth = format!(
                r#"{{"cmd":"AUTHORIZE","nonce":"raw-test-nonce","args":{{"client_id":"{client_id}","scopes":["rpc","rpc.voice.read","rpc.voice.write"]}}}}"#
            );
            let mut f = Vec::new();
            f.extend(&1u32.to_le_bytes());
            f.extend(&(auth.len() as u32).to_le_bytes());
            f.extend(auth.as_bytes());
            println!("\nsending AUTHORIZE: {auth}");
            pipe.write_all(&f).await.expect("write authorize");
            println!("sent. A modal should appear in Discord — accept it.\n");
        }

        let mut header = [0u8; 8];
        match tokio::time::timeout(
            std::time::Duration::from_secs(45),
            pipe.read_exact(&mut header),
        )
        .await
        {
            Ok(Ok(_)) => {
                let op = u32::from_le_bytes(header[0..4].try_into().unwrap());
                let len = u32::from_le_bytes(header[4..8].try_into().unwrap());
                let mut body = vec![0u8; len as usize];
                pipe.read_exact(&mut body).await.expect("body");
                println!("frame {n}: opcode={op} len={len}");
                // The AUTHORIZE reply carries a live OAuth code. It is single-use and
                // short-lived, but printing secrets in a tool meant for bug reports invites
                // pasting them into issues.
                let text = String::from_utf8_lossy(&body);
                println!("  {}", redact_code(&text));
            }
            Ok(Err(e)) => {
                println!("frame {n}: read error: {e}");
                break;
            }
            Err(_) => {
                println!("frame {n}: TIMEOUT, Discord sent nothing in 45 s");
                break;
            }
        }
    }
}
