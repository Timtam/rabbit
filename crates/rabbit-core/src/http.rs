//! Shared HTTP helpers: GitHub-API authentication used by the latest-version
//! and artifact-resolver paths so unauthenticated CI runners do not blow the
//! 60-requests-per-hour shared rate limit.

use std::env;
use std::sync::Once;

use reqwest::blocking::{ClientBuilder, RequestBuilder};

const GITHUB_API_HOST: &str = "api.github.com";

/// Start every RABBIT HTTP client here instead of `Client::builder()`.
///
/// reqwest is built with `rustls-no-provider`, so building a client panics
/// (an abort in release builds) unless a process-wide rustls crypto provider
/// is installed first. Installing it here, on first use, covers the CLI, the
/// wizard's worker threads, the download pool and the tests alike; a call in
/// `main` would miss the tests.
pub(crate) fn client_builder() -> ClientBuilder {
    static INSTALL_CRYPTO_PROVIDER: Once = Once::new();
    INSTALL_CRYPTO_PROVIDER.call_once(|| {
        // An error only means a provider is already installed for this
        // process, and reqwest uses that one.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    let builder = reqwest::blocking::Client::builder();
    // Certificates are checked against the system's trust store. A Windows
    // PC with automatic root certificate updates turned off can lack a root
    // a download host needs, which the Mozilla roots RABBIT used to bundle
    // covered. Hand those over as extra roots: Windows falls back to them
    // only when it can't build a trusted chain itself, so roots that
    // antivirus or company software put in the store still come first.
    // macOS ships these roots already.
    #[cfg(windows)]
    let builder = builder.tls_certs_merge(
        webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .filter_map(|der| reqwest::Certificate::from_der(der).ok()),
    );
    builder
}

/// If the request targets `api.github.com` and a `GITHUB_TOKEN` is exported
/// (the standard CI secret), attach `Authorization: Bearer <token>` to the
/// request so we get the 5000-requests-per-hour authenticated quota instead
/// of the unauthenticated 60-per-hour cap.
pub(crate) fn maybe_apply_github_auth(builder: RequestBuilder, url: &str) -> RequestBuilder {
    if !is_github_api_url(url) {
        return builder;
    }
    let Ok(token) = env::var(rabbit_platform::process::GITHUB_TOKEN_ENV) else {
        return builder;
    };
    if token.trim().is_empty() {
        return builder;
    }
    builder.header("Authorization", format!("Bearer {}", token.trim()))
}

fn is_github_api_url(url: &str) -> bool {
    url.starts_with("https://api.github.com/")
        || url.starts_with(&format!("https://{GITHUB_API_HOST}/"))
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::{client_builder, is_github_api_url};

    /// The download pool builds a client per worker at the same moment, so
    /// the first provider install must be safe to race.
    #[test]
    fn clients_build_from_many_threads_at_once() {
        let workers: Vec<_> = (0..8)
            .map(|_| thread::spawn(|| client_builder().build().map(drop)))
            .collect();
        for worker in workers {
            worker
                .join()
                .expect("building a client must not panic")
                .expect("client builds");
        }
        assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    }

    /// Answer `responses.len()` connections in turn on `listener`, each with
    /// its canned response, and return the request heads received.
    fn serve(listener: TcpListener, responses: Vec<String>) -> thread::JoinHandle<Vec<String>> {
        thread::spawn(move || {
            let mut heads = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap() == 1 {
                    head.push(byte[0]);
                }
                heads.push(String::from_utf8_lossy(&head).to_ascii_lowercase());
                stream.write_all(response.as_bytes()).unwrap();
            }
            heads
        })
    }

    fn redirect_to(location: &str) -> String {
        format!(
            "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
    }

    const OK: &str = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";

    /// The GitHub token only ever goes to api.github.com, whose release
    /// assets redirect to other hosts: the client must drop Authorization on
    /// a redirect to another host, and keep it on the same one.
    #[test]
    fn authorization_does_not_follow_a_redirect_to_another_host() {
        let other = TcpListener::bind("127.0.0.1:0").unwrap();
        let other_url = format!("http://{}/asset", other.local_addr().unwrap());
        let other_server = serve(other, vec![OK.to_string()]);
        let origin = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin_addr = origin.local_addr().unwrap();
        let origin_server = serve(
            origin,
            vec![
                redirect_to(&other_url),
                redirect_to(&format!("http://{origin_addr}/asset")),
                OK.to_string(),
            ],
        );
        let client = client_builder().build().unwrap();

        let response = client
            .get(format!("http://{origin_addr}/to-other-host"))
            .header("Authorization", "Bearer secret")
            .send()
            .unwrap();
        assert_eq!(response.text().unwrap(), "ok");
        let response = client
            .get(format!("http://{origin_addr}/to-same-host"))
            .header("Authorization", "Bearer secret")
            .send()
            .unwrap();
        assert_eq!(response.text().unwrap(), "ok");

        let other_heads = other_server.join().unwrap();
        assert!(
            !other_heads[0].contains("authorization:"),
            "{}",
            other_heads[0]
        );
        let origin_heads = origin_server.join().unwrap();
        assert!(
            origin_heads[2].contains("authorization: bearer secret"),
            "{}",
            origin_heads[2]
        );
    }

    #[test]
    fn detects_github_api_urls() {
        assert!(is_github_api_url(
            "https://api.github.com/repos/cfillion/reapack/releases/latest"
        ));
        assert!(!is_github_api_url(
            "https://github.com/jcsteh/osara/releases/download/snapshots/osara_2026.zip"
        ));
        assert!(!is_github_api_url("https://www.reaper.fm/download.php"));
    }
}
