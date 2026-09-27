use std::{
    env, fs, io,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use zhorten_server::{
    ServerOptions, SledServerOptions,
    sled_db::{Database, Error},
    sled_router,
};

const CODE: &str = "bench";
const TARGET_URL: &str = "https://example.com/bench";

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let duration = env_duration("ZHORTEN_REDIRECT_BENCH_SECONDS", 10);
    let concurrency = env_usize("ZHORTEN_REDIRECT_BENCH_CONCURRENCY", 64);
    let cache_capacity = env_u64("ZHORTEN_REDIRECT_BENCH_CACHE_CAPACITY", 64 * 1024 * 1024);

    let temp = TempDir::new("zhorten-redirect-bench")?;
    let database = Database::open(temp.path().join("zhorten.db"), cache_capacity)?;
    seed_link(&database).await?;

    let app = sled_router(
        database,
        SledServerOptions {
            server: ServerOptions {
                site_root: temp.path().join("site"),
                username: "admin".into(),
                password: "benchmark-password".into(),
                secure_cookies: false,
            },
            mcp_token: None,
            mcp_hosts: Vec::new(),
        },
    );
    write_site_root(&temp.path().join("site"))?;

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("benchmark server failed");
    });

    warm_up(address).await?;

    let deadline = Instant::now() + duration;
    let total = Arc::new(AtomicU64::new(0));
    let started = Instant::now();
    let mut workers = Vec::with_capacity(concurrency);
    for _ in 0..concurrency {
        let total = Arc::clone(&total);
        workers.push(tokio::spawn(async move {
            run_worker(address, deadline, total).await
        }));
    }

    for worker in workers {
        worker.await??;
    }
    let elapsed = started.elapsed();
    server.abort();

    let requests = total.load(Ordering::Relaxed);
    let rps = requests as f64 / elapsed.as_secs_f64();
    println!("redirect benchmark");
    println!("  route: /{CODE}");
    println!("  storage: sled");
    println!("  server: axum over TCP keep-alive");
    println!("  concurrency: {concurrency}");
    println!("  duration: {:.3}s", elapsed.as_secs_f64());
    println!("  requests: {requests}");
    println!("  throughput: {rps:.2} req/s");

    Ok(())
}

async fn seed_link(database: &Database) -> Result<(), Box<dyn std::error::Error>> {
    zhorten_service::create_link(database, CODE.to_owned(), TARGET_URL, 0)
        .await
        .map_err(|error| match error {
            zhorten_service::CreateLinkError::Database(error) => error,
            other => Error::InvalidRecord(serde_json::Error::io(io::Error::other(format!(
                "seed failed: {other:?}"
            )))),
        })?;
    Ok(())
}

async fn warm_up(address: SocketAddr) -> io::Result<()> {
    let mut stream = TcpStream::connect(address).await?;
    request_redirect(&mut stream).await?;
    Ok(())
}

async fn run_worker(
    address: SocketAddr,
    deadline: Instant,
    total: Arc<AtomicU64>,
) -> io::Result<()> {
    let mut stream = TcpStream::connect(address).await?;
    let mut completed = 0;
    while Instant::now() < deadline {
        request_redirect(&mut stream).await?;
        completed += 1;
    }
    total.fetch_add(completed, Ordering::Relaxed);
    Ok(())
}

async fn request_redirect(stream: &mut TcpStream) -> io::Result<()> {
    stream
        .write_all(b"GET /bench HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n")
        .await?;

    let mut response = Vec::with_capacity(512);
    let mut byte = [0; 1];
    while !response.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).await?;
        response.push(byte[0]);
    }
    if !response.starts_with(b"HTTP/1.1 307") {
        return Err(io::Error::other(format!(
            "unexpected response: {}",
            String::from_utf8_lossy(&response)
        )));
    }
    Ok(())
}

fn write_site_root(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path.join("assets"))?;
    fs::write(
        path.join("index.html"),
        "<!doctype html><title>bench</title>",
    )?;
    Ok(())
}

fn env_duration(name: &str, default_secs: u64) -> Duration {
    Duration::from_secs(env_u64(name, default_secs))
}

fn env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> io::Result<Self> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = env::temp_dir().join(format!("{prefix}-{}-{timestamp}", std::process::id()));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
