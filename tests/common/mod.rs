use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use reqwest::StatusCode;

static SERVER: OnceLock<TestServer> = OnceLock::new();

pub struct TestServer {
    pub port: u16,
    pub base_url: String,
    child: Child,
}

impl TestServer {
    fn spawn() -> Result<Self> {
        // Pick an unused port for isolation
        let port = portpicker::pick_unused_port().context("failed to pick free port")?;
        let base_url = format!("http://127.0.0.1:{}", port);

        // Spawn the already-built binary to keep start fast during tests
        // Assumes debug profile; adjust if you run tests with --release
        let mut cmd = Command::new("target/debug/monk-api-rust");
        cmd.env("MONK_API_PORT", port.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        // Inherit environment so the server can see DATABASE_URL and MONK_TENANT_DB from .env (loaded by the server)
        let child = cmd.spawn().context("failed to spawn server binary")?;

        Ok(Self { port, base_url, child })
    }

    async fn wait_ready(&self, timeout: Duration) -> Result<()> {
        let client = reqwest::Client::new();
        let deadline = Instant::now() + timeout;
        loop {
            if Instant::now() > deadline { break; }
            let url = format!("{}/health", self.base_url);
            match client.get(&url).send().await {
                Ok(resp) => {
                    // Consider server ready on any non-404 response
                    if resp.status() == StatusCode::OK || resp.status() == StatusCode::SERVICE_UNAVAILABLE {
                        return Ok(());
                    }
                }
                Err(_) => {}
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        anyhow::bail!("server did not become ready on {} within {:?}", self.base_url, timeout)
    }
}

pub async fn ensure_server() -> Result<&'static TestServer> {
    // Use stable get_or_init and convert init errors into a panic with context.
    let server = SERVER.get_or_init(|| TestServer::spawn().expect("failed to spawn server binary"));
    server.wait_ready(Duration::from_secs(10)).await?;
    Ok(server)
}

