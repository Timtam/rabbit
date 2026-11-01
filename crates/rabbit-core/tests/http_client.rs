//! A process of its own, so no rustls crypto provider is installed before
//! RABBIT builds its first HTTP client, which is the release binary's
//! situation. reqwest panics (an abort in release builds) when it finds none.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use rabbit_core::RabbitError;

#[test]
fn the_first_client_in_a_process_gets_a_crypto_provider() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap() == 1 {
            head.push(byte[0]);
        }
        stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });

    let result = rabbit_core::self_update::fetch_self_update_manifest(&format!(
        "http://{address}/manifest.json"
    ));

    assert!(
        matches!(result, Err(RabbitError::Http { .. })),
        "a 404 comes back as an HTTP error, not a panic: {result:?}"
    );
    assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    server.join().unwrap();
}
