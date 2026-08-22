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
        Self::start_with_extra_options(major_version, "").await
    }

    /// Unit U51: same as [`Self::start`], but `extra_options` is
    /// appended verbatim to `pg_ctl start`'s own `-o` GUC-override
    /// string — the only way to set a start-only (not reloadable) GUC
    /// like `shared_preload_libraries`, needed to make an extension
    /// such as `pg_stat_statements` genuinely loadable in a test.
    pub async fn start_with_extra_options(major_version: u32, extra_options: &str) -> Self {
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

        Self::start_pg_ctl(
            major_version,
            data_dir,
            bin_dir,
            lib_dir,
            superuser,
            extra_options,
        )
        .await
    }

    /// Unit U52: a real streaming-replication standby of `self`, built
    /// via a real `pg_basebackup -R` against this instance's own
    /// socket — never a fixture. Works entirely over the same
    /// Unix-domain socket every `TestPostgres` already uses
    /// (`primary_conninfo`'s `host` accepts a socket directory, no TCP
    /// listener needed); `initdb --auth=trust`'s own default
    /// `pg_hba.conf` already trust-authenticates `replication`
    /// connections, so no extra auth setup is needed either. `-R`
    /// auto-writes `standby.signal` + `primary_conninfo`; the default
    /// `--wal-method=stream` (since PG10) needs no extra flags for a
    /// consistent, working standby. Returns another full
    /// `TestPostgres` — from every caller's perspective a standby is
    /// just another real running instance, so every existing method
    /// (`superuser_client`, `config`, `stop`) works unchanged.
    pub async fn start_standby_of(&self, major_version: u32) -> Self {
        let bin_dir = bin_dir(major_version);
        if !bin_dir.join("pg_basebackup").exists() {
            panic!(
                "PostgreSQL {major_version} binaries not found at {}; run \
                 scripts/setup-test-postgres.sh first",
                bin_dir.display()
            );
        }
        let lib_dir = libpq_dir();
        let data_dir = tempfile::tempdir().expect("tempdir");
        let pgdata = data_dir.path().join("data");

        let status = Command::new(bin_dir.join("pg_basebackup"))
            .arg("-h")
            .arg(&self.socket_dir)
            .arg("-p")
            .arg(self.port.to_string())
            .arg("-U")
            .arg(&self.superuser)
            .arg("-D")
            .arg(&pgdata)
            .arg("-R")
            .env("LD_LIBRARY_PATH", &lib_dir)
            .status()
            .await
            .expect("spawn pg_basebackup");
        assert!(
            status.success(),
            "pg_basebackup failed for PostgreSQL {major_version}"
        );

        Self::start_pg_ctl(
            major_version,
            data_dir,
            bin_dir,
            lib_dir,
            self.superuser.clone(),
            "",
        )
        .await
    }

    /// Shared tail of every start path above: run `pg_ctl start`
    /// against an already-prepared `pgdata` directory (built by either
    /// `initdb` or `pg_basebackup`), wait for real readiness, and
    /// construct `Self`.
    async fn start_pg_ctl(
        major_version: u32,
        data_dir: TempDir,
        bin_dir: PathBuf,
        lib_dir: PathBuf,
        superuser: String,
        extra_options: &str,
    ) -> Self {
        let pgdata = data_dir.path().join("data");
        let port = pick_free_port();
        let log_path = data_dir.path().join("server.log");
        let status = Command::new(bin_dir.join("pg_ctl"))
            .arg("-D")
            .arg(&pgdata)
            .arg("-l")
            .arg(&log_path)
            .arg("-o")
            .arg(format!(
                "-p {port} -k {} -c listen_addresses='' {extra_options}",
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

/// A real, throwaway child process, plus its real `start_time_ticks`
/// (`/proc/[pid]/stat` field 22 — the same field
/// `core::actions::host::ProcessTargetVerifier` itself reads for real
/// target-identity verification) — a real target `policy_endpoints.rs`'s
/// and `tuning_endpoints.rs`'s own tests can genuinely act on (unit
/// U22/U23). Shared here rather than duplicated per test file.
pub struct SpawnedTarget {
    child: std::process::Child,
    pub pid: i64,
    pub start_time_ticks: i64,
}

impl SpawnedTarget {
    pub fn spawn() -> Self {
        let child = StdCommand::new("sleep")
            .arg("300")
            .spawn()
            .expect("spawn sleep");
        let pid = child.id() as i64;
        let start_time_ticks = read_start_time_ticks(pid);
        Self {
            child,
            pid,
            start_time_ticks,
        }
    }

    pub fn target_identity_json(&self) -> String {
        serde_json::json!({
            "kind": "PROCESS",
            "pid": self.pid,
            "start_time_ticks": self.start_time_ticks,
        })
        .to_string()
    }

    pub fn kill_and_reap(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for SpawnedTarget {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Field 22 of `/proc/[pid]/stat`, skipping past the `comm` field's own
/// closing `)` (which can itself contain spaces/parens) before splitting
/// the remainder on whitespace — the same real technique
/// `core::telemetry::process::read_start_time_ticks` uses, reimplemented
/// here since that function is `pub(crate)` to `core` and this is a
/// different crate's test.
fn read_start_time_ticks(pid: i64) -> i64 {
    let raw = std::fs::read_to_string(format!("/proc/{pid}/stat")).expect("read /proc/[pid]/stat");
    let after_comm = raw.rsplit_once(')').expect("stat has a comm field").1;
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    // fields[0] is `state` (overall field 3); `starttime` is overall
    // field 22, i.e. fields[22 - 3] = fields[19].
    fields[19].parse().expect("starttime is a valid integer")
}
