//! Spins up a real PostgreSQL server instance for dbms integration tests —
//! not Docker (this dev sandbox doesn't have it), the extracted-binary
//! technique from `scripts/setup-test-postgres.sh` (PGDG `.deb`s,
//! `dpkg -x`, no root). See the U4 plan's "Environment realities" section.
//!
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not `tests/*.rs`.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::io::Read;
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
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

/// `libpq.so.5` lives in a separate shared package (see
/// `scripts/setup-test-postgres.sh`) — located by walking the shared
/// install dir rather than hardcoding a multiarch triplet.
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
    /// Starts a fresh instance with trust auth (this is a throwaway,
    /// locally-socket-only test fixture, not a real deployment — trust
    /// auth here is not the same class of decision as it would be for the
    /// tool's own default `sslmode`/auth handling, which requirement 2
    /// covers separately and for real).
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

    /// Like `start`, but with a real self-signed cert and `ssl=on` —
    /// PostgreSQL only ever negotiates TLS over TCP (never over a Unix
    /// socket, which has no network exposure to protect in the first
    /// place), so this also enables `listen_addresses='127.0.0.1'`, unlike
    /// every other fixture in this harness which stays socket-only.
    pub async fn start_with_tls(major_version: u32) -> Self {
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

        let cert_path = pgdata.join("server.crt");
        let key_path = pgdata.join("server.key");
        let status = StdCommand::new("openssl")
            .args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout"])
            .arg(&key_path)
            .arg("-out")
            .arg(&cert_path)
            .args(["-days", "1", "-subj", "/CN=localhost"])
            // Unit U77: openssl's own `req -x509` default sets
            // `basicConstraints=CA:TRUE` (a legacy compatibility default,
            // confirmed empirically) — real chain validation
            // (`rustls-webpki`) rejects any end-entity/leaf certificate
            // that claims to be a CA (`Error::CaUsedAsEndEntity`), which
            // this server cert always is. Explicit `CA:FALSE` makes this
            // a genuine leaf cert, matching real-world server certs.
            .args(["-addext", "basicConstraints=critical,CA:FALSE"])
            // `rustls-webpki` requires a real `subjectAltName` for
            // hostname matching — it never falls back to the legacy `CN`
            // field the way older verifiers did (confirmed empirically:
            // without this, hostname verification fails even for a
            // genuinely matching `localhost`, reporting zero presented
            // names). `/CN=localhost` alone is therefore not enough.
            .args(["-addext", "subjectAltName=DNS:localhost"])
            .status()
            .expect("spawn openssl req");
        assert!(
            status.success(),
            "openssl self-signed cert generation failed"
        );
        // PostgreSQL refuses to start if the private key is group/world
        // readable.
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
            .expect("chmod server.key");

        let log_path = data_dir.path().join("server.log");
        let status = Command::new(bin_dir.join("pg_ctl"))
            .arg("-D")
            .arg(&pgdata)
            .arg("-l")
            .arg(&log_path)
            .arg("-o")
            .arg(format!(
                "-p {port} -k {} -c listen_addresses=127.0.0.1 -c ssl=on \
                 -c ssl_cert_file={} -c ssl_key_file={}",
                data_dir.path().display(),
                cert_path.display(),
                key_path.display(),
            ))
            .arg("start")
            .env("LD_LIBRARY_PATH", &lib_dir)
            .status()
            .await
            .expect("spawn pg_ctl start");
        assert!(
            status.success(),
            "pg_ctl start failed for TLS-enabled PostgreSQL {major_version}; log:\n{}",
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

    /// A `tokio_postgres::Config` over real TCP to `127.0.0.1` — only
    /// meaningful against an instance started via `start_with_tls` (see
    /// its doc comment for why plain `start`'s Unix-socket-only instances
    /// have no TLS to speak of at all).
    pub fn tcp_config(&self, user: &str, dbname: &str) -> tokio_postgres::Config {
        let mut cfg = tokio_postgres::Config::new();
        cfg.host("127.0.0.1");
        cfg.port(self.port);
        cfg.user(user);
        cfg.dbname(dbname);
        cfg
    }

    /// The real self-signed cert `start_with_tls` generated for this
    /// instance (unit U77) — since it's self-signed, this same file is
    /// its own trust anchor, usable directly as a `verify-ca`/`verify-
    /// full` CA bundle in tests. Only meaningful for an instance started
    /// via `start_with_tls`.
    pub fn tls_server_cert_path(&self) -> PathBuf {
        self.socket_dir.join("data").join("server.crt")
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

    /// A real `tokio_postgres::Config` pointed at this instance over its
    /// Unix socket — the same driver the adapter itself uses, not a
    /// separate psql-based path, so setup queries exercise the identical
    /// wire behavior the adapter will see.
    pub fn config(&self, user: &str, dbname: &str) -> tokio_postgres::Config {
        let mut cfg = tokio_postgres::Config::new();
        cfg.host_path(&self.socket_dir);
        cfg.port(self.port);
        cfg.user(user);
        cfg.dbname(dbname);
        cfg
    }

    /// Clones this instance into a real streaming standby via
    /// `pg_basebackup -R` (which writes `standby.signal` +
    /// `primary_conninfo` into the copy automatically — the real,
    /// documented way to stand up a PostgreSQL streaming replica, not a
    /// hand-rolled recovery-config approximation of it), started on its
    /// own port. `self` must have `wal_level=replica` (PostgreSQL's
    /// default since PG13, so not set explicitly here) and permit
    /// replication connections under the same trust auth `start()` set up
    /// for normal connections.
    pub async fn start_streaming_replica(&self) -> TestPostgres {
        let bin_dir = bin_dir(self.major_version);
        let data_dir = tempfile::tempdir().expect("tempdir");
        let pgdata = data_dir.path().join("data");
        let port = pick_free_port();

        let status = Command::new(bin_dir.join("pg_basebackup"))
            .arg("-h")
            .arg(&self.socket_dir)
            .arg("-p")
            .arg(self.port.to_string())
            .arg("-U")
            .arg(&self.superuser)
            .arg("-D")
            .arg(&pgdata)
            .arg("-Fp") // plain-format data directory copy, not a tarball
            .arg("-Xs") // stream the WAL needed to make the base backup consistent
            .arg("-R") // write standby.signal + primary_conninfo automatically
            .env("LD_LIBRARY_PATH", &self.lib_dir)
            .status()
            .await
            .expect("spawn pg_basebackup");
        assert!(status.success(), "pg_basebackup failed");

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
            .env("LD_LIBRARY_PATH", &self.lib_dir)
            .status()
            .await
            .expect("spawn pg_ctl start (replica)");
        assert!(
            status.success(),
            "pg_ctl start failed for the replica; log:\n{}",
            std::fs::read_to_string(&log_path).unwrap_or_default()
        );

        let replica = TestPostgres {
            major_version: self.major_version,
            port,
            socket_dir: data_dir.path().to_path_buf(),
            superuser: self.superuser.clone(),
            data_dir,
            bin_dir,
            lib_dir: self.lib_dir.clone(),
            stopped: false,
        };
        replica.wait_until_ready().await;
        replica
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

    /// `pg_ctl stop -m immediate` — a hard stop (no checkpoint, no
    /// graceful backend shutdown), matching what an actual unexpected
    /// disconnect looks like for the mid-poll-disconnect test, and a fast,
    /// reliable teardown for every other test.
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

    /// Restarts a stopped instance on the *same* data directory, port, and
    /// socket dir — the mid-poll-disconnect test needs the pool's existing
    /// connection config to still point at something real afterward, not a
    /// freshly constructed instance on a new port.
    pub async fn start_again(&mut self) {
        assert!(
            self.stopped,
            "start_again called on an instance that was never stopped"
        );
        let pgdata = self.data_dir.path().join("data");
        let log_path = self.data_dir.path().join("server.log");
        let status = Command::new(self.bin_dir.join("pg_ctl"))
            .arg("-D")
            .arg(&pgdata)
            .arg("-l")
            .arg(&log_path)
            .arg("-o")
            .arg(format!(
                "-p {} -k {} -c listen_addresses=''",
                self.port,
                self.socket_dir.display()
            ))
            .arg("start")
            .env("LD_LIBRARY_PATH", &self.lib_dir)
            .status()
            .await
            .expect("spawn pg_ctl start (restart)");
        assert!(status.success(), "pg_ctl start (restart) failed");
        self.stopped = false;
        self.wait_until_ready().await;
    }
}

impl Drop for TestPostgres {
    /// Best-effort synchronous backstop for a test that panics before
    /// reaching an explicit `.stop().await` — `Drop` can't be async, so
    /// this reads the real postmaster PID PostgreSQL itself writes to
    /// `$PGDATA/postmaster.pid` and sends it a direct kill, rather than
    /// re-invoking `pg_ctl` (which would need its own async runtime).
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
