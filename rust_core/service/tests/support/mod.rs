//! Spins up a real, throwaway PostgreSQL instance for
//! `correlation_endpoints.rs` — not Docker (this dev sandbox doesn't have
//! it), the same extracted-binary technique
//! `scripts/setup-test-postgres.sh` sets up and `core/tests/dbms/common/
//! mod.rs` already uses for `core`'s own DB integration tests. Trimmed to
//! exactly what this crate's tests need (start/stop/connect); Cargo
//! integration test binaries in different packages can't share code
//! across a `tests/` directory, so this is a deliberately small, scoped
//! duplicate rather than a new shared test-support crate — see
//! docs/adr/0025.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::io::Read;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::time::Duration;

use tempfile::TempDir;
use tokio::process::Command;
use tokio::time::{sleep, Instant};

fn install_root() -> PathBuf {
    std::env::var("PG_TEST_INSTALL_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home =
                std::env::var("HOME").expect("HOME must be set to find test PostgreSQL binaries");
            PathBuf::from(home).join(".local-tools")
        })
}

fn bin_dir(major_version: u32) -> PathBuf {
    install_root()
        .join(format!("pg{major_version}"))
        .join("usr/lib/postgresql")
        .join(major_version.to_string())
        .join("bin")
}

fn libpq_dir() -> PathBuf {
    let shared = install_root().join("pg-shared");
    fn find(dir: &std::path::Path) -> Option<PathBuf> {
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find(&path) {
                    return Some(found);
                }
            } else if path.file_name().and_then(|n| n.to_str()) == Some("libpq.so.5") {
                return path.parent().map(PathBuf::from);
            }
        }
        None
    }
    find(&shared).unwrap_or_else(|| {
        panic!(
            "libpq.so.5 not found under {}; run scripts/setup-test-postgres.sh first",
            shared.display()
        )
    })
}

fn pick_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local_addr").port()
}

pub struct TestPostgres {
    pub major_version: u32,
    pub port: u16,
    pub socket_dir: PathBuf,
    pub superuser: String,
    data_dir: TempDir,
    bin_dir: PathBuf,
    lib_dir: PathBuf,
    stopped: bool,
}

impl TestPostgres {
    /// Starts a fresh instance with trust auth — a throwaway,
    /// socket-only test fixture, not a real deployment.
    pub async fn start(major_version: u32) -> Self {
        let bin_dir = bin_dir(major_version);
        if !bin_dir.join("initdb").exists() {
            panic!(
                "PostgreSQL {major_version} binaries not found at {}; run \
                 scripts/setup-test-postgres.sh first",
                bin_dir.display()
            );
        }
        let lib_dir = libpq_dir();
        let data_dir = tempfile::tempdir().expect("tempdir");
        let pgdata = data_dir.path().join("data");
        let port = pick_free_port();
        let superuser = "postgres".to_string();

        let status = Command::new(bin_dir.join("initdb"))
            .arg("-D")
            .arg(&pgdata)
            .arg("-U")
            .arg(&superuser)
            .arg("--auth=trust")
            .env("LD_LIBRARY_PATH", &lib_dir)
            .status()
            .await
            .expect("spawn initdb");
        assert!(
            status.success(),
            "initdb failed for PostgreSQL {major_version}"
        );

        let log_path = data_dir.path().join("server.log");
        let status = Command::new(bin_dir.join("pg_ctl"))
            .arg("-D")
            .arg(&pgdata)
            .arg("-l")
            .arg(&log_path)
            .arg("-o")
            .arg(format!(
                "-p {port} -k {} -c listen_addresses=''",
                data_dir.path().display()
            ))
            .arg("start")
            .env("LD_LIBRARY_PATH", &lib_dir)
            .status()
            .await
            .expect("spawn pg_ctl start");
        assert!(
            status.success(),
            "pg_ctl start failed for PostgreSQL {major_version}; log:\n{}",
            std::fs::read_to_string(&log_path).unwrap_or_default()
        );

        let instance = Self {
            major_version,
            port,
            socket_dir: data_dir.path().to_path_buf(),
            superuser,
            data_dir,
            bin_dir,
            lib_dir,
            stopped: false,
        };
        instance.wait_until_ready().await;
        instance
    }

    async fn wait_until_ready(&self) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let status = Command::new(self.bin_dir.join("pg_isready"))
                .arg("-h")
                .arg(&self.socket_dir)
                .arg("-p")
                .arg(self.port.to_string())
                .env("LD_LIBRARY_PATH", &self.lib_dir)
                .status()
                .await;
            if matches!(status, Ok(s) if s.success()) {
                return;
            }
            if Instant::now() >= deadline {
                panic!(
                    "PostgreSQL {} did not become ready within the deadline",
                    self.major_version
                );
            }
            sleep(Duration::from_millis(100)).await;
        }
    }

    /// A real `tokio_postgres::Config` over this instance's Unix socket.
    pub fn config(&self, user: &str, dbname: &str) -> tokio_postgres::Config {
        let mut cfg = tokio_postgres::Config::new();
        cfg.host_path(&self.socket_dir);
        cfg.port(self.port);
        cfg.user(user);
        cfg.dbname(dbname);
        cfg
    }

    pub async fn superuser_client(&self) -> tokio_postgres::Client {
        let (client, connection) = self
            .config(&self.superuser.clone(), &self.superuser.clone())
            .connect(tokio_postgres::NoTls)
            .await
            .expect("connect as superuser");
        tokio::spawn(async move {
            let _ = connection.await;
        });
        client
    }

    pub async fn stop(&mut self) {
        if self.stopped {
            return;
        }
        let pgdata = self.data_dir.path().join("data");
        let _ = Command::new(self.bin_dir.join("pg_ctl"))
            .arg("-D")
            .arg(&pgdata)
            .arg("-m")
            .arg("immediate")
            .arg("stop")
            .env("LD_LIBRARY_PATH", &self.lib_dir)
            .status()
            .await;
        self.stopped = true;
    }
}

impl Drop for TestPostgres {
    /// Best-effort synchronous backstop for a test that panics before
    /// reaching an explicit `.stop().await` — `Drop` can't be async, so
    /// this reads the real postmaster PID PostgreSQL itself writes to
    /// `$PGDATA/postmaster.pid` and sends it a direct kill.
    fn drop(&mut self) {
        if self.stopped {
            return;
        }
        let pid_file = self.data_dir.path().join("data").join("postmaster.pid");
        let Ok(mut file) = std::fs::File::open(&pid_file) else {
            return;
        };
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_err() {
            return;
        }
        let Some(pid_line) = contents.lines().next() else {
            return;
        };
        if let Ok(pid) = pid_line.trim().parse::<u32>() {
            let _ = StdCommand::new("kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .status();
        }
    }
}
