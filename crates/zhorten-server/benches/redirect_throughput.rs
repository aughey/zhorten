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
use zhorten_core::cli::{AnalyticsMode, DatabaseArgs, DatabaseBackend};
use zhorten_database::SharedDatabase;
use zhorten_server::{LocalServerOptions, ServerOptions, local_router};

const CODE: &str = "bench";
const TARGET_URL: &str = "https://example.com/bench";

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let duration = env_duration("ZHORTEN_REDIRECT_BENCH_SECONDS", 10);
    let concurrency = env_usize("ZHORTEN_REDIRECT_BENCH_CONCURRENCY", 64);
    let cache_capacity = env_u64("ZHORTEN_REDIRECT_BENCH_CACHE_CAPACITY", 64 * 1024 * 1024);
    let backends = env_backends("ZHORTEN_REDIRECT_BENCH_BACKEND")?;
    let layers = env_layers("ZHORTEN_REDIRECT_BENCH_LAYER")?;

    println!("redirect benchmark");
    println!("  route: /{CODE}");
    println!("  concurrency: {concurrency}");
    println!("  requested duration: {}s", duration.as_secs());

    for backend in backends {
        for layer in &layers {
            let result =
                run_benchmark(backend, *layer, duration, concurrency, cache_capacity).await?;
            println!();
            println!("  storage: {}", backend_name(backend));
            println!("  layer: {}", layer.name());
            println!("  duration: {:.3}s", result.elapsed.as_secs_f64());
            println!("  requests: {}", result.requests);
            println!("  throughput: {:.2} req/s", result.requests_per_second());
        }
    }

    Ok(())
}

async fn run_benchmark(
    backend: DatabaseBackend,
    layer: BenchLayer,
    duration: Duration,
    concurrency: usize,
    cache_capacity: u64,
) -> Result<BenchResult, Box<dyn std::error::Error>> {
    let temp = TempDir::new("zhorten-redirect-bench")?;
    let config = DatabaseArgs {
        backend,
        database: temp
            .path()
            .join(database_name(backend))
            .display()
            .to_string(),
        analytics: AnalyticsMode::Disabled,
        cache_capacity,
    };
    let database = zhorten_database::open(&config)?;
    seed_link(&database).await?;

    match layer {
        BenchLayer::Axum => run_axum_benchmark(temp, database, duration, concurrency).await,
        BenchLayer::Service => run_service_benchmark(database, duration, concurrency).await,
    }
}

async fn run_axum_benchmark(
    temp: TempDir,
    database: SharedDatabase,
    duration: Duration,
    concurrency: usize,
) -> Result<BenchResult, Box<dyn std::error::Error>> {
    write_site_root(&temp.path().join("site"))?;

    let app = local_router(
        database,
        LocalServerOptions {
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

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
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
    Ok(BenchResult { elapsed, requests })
}

async fn run_service_benchmark(
    database: SharedDatabase,
    duration: Duration,
    concurrency: usize,
) -> Result<BenchResult, Box<dyn std::error::Error>> {
    warm_up_service(&database).await?;

    let deadline = Instant::now() + duration;
    let total = Arc::new(AtomicU64::new(0));
    let started = Instant::now();
    let mut workers = Vec::with_capacity(concurrency);
    for _ in 0..concurrency {
        let database = Arc::clone(&database);
        let total = Arc::clone(&total);
        workers.push(tokio::spawn(async move {
            run_service_worker(database, deadline, total).await
        }));
    }

    for worker in workers {
        worker.await??;
    }
    let elapsed = started.elapsed();
    let requests = total.load(Ordering::Relaxed);
    Ok(BenchResult { elapsed, requests })
}

async fn seed_link(database: &SharedDatabase) -> Result<(), Box<dyn std::error::Error>> {
    zhorten_service::create_link(database, CODE.to_owned(), TARGET_URL, 0)
        .await
        .map_err(|error| match error {
            zhorten_service::CreateLinkError::Database(error) => io::Error::other(error),
            other => io::Error::other(format!("seed failed: {other:?}")),
        })?;
    Ok(())
}

async fn warm_up(address: SocketAddr) -> io::Result<()> {
    let mut stream = TcpStream::connect(address).await?;
    request_redirect(&mut stream).await?;
    Ok(())
}

async fn warm_up_service(database: &SharedDatabase) -> io::Result<()> {
    follow_service(database).await?;
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

async fn run_service_worker(
    database: SharedDatabase,
    deadline: Instant,
    total: Arc<AtomicU64>,
) -> io::Result<()> {
    let mut completed = 0;
    while Instant::now() < deadline {
        follow_service(&database).await?;
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

async fn follow_service(database: &SharedDatabase) -> io::Result<()> {
    let url = zhorten_service::follow_link(
        database,
        CODE.to_owned(),
        zhorten_service::ClickContext::new(0),
    )
    .await
    .map_err(|error| match error {
        zhorten_service::FollowLinkError::Database(error) => io::Error::other(error),
        other => io::Error::other(format!("follow failed: {other:?}")),
    })?;
    if url != TARGET_URL {
        return Err(io::Error::other(format!(
            "unexpected redirect target: {url}"
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

fn env_backends(name: &str) -> io::Result<Vec<DatabaseBackend>> {
    match env::var(name).as_deref() {
        Ok("sled") => Ok(vec![DatabaseBackend::Sled]),
        Ok("sqlite") => Ok(vec![DatabaseBackend::Sqlite]),
        Ok("all") | Err(_) => Ok(vec![DatabaseBackend::Sled, DatabaseBackend::Sqlite]),
        Ok(value) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be sled, sqlite, or all; got {value}"),
        )),
    }
}

fn env_layers(name: &str) -> io::Result<Vec<BenchLayer>> {
    match env::var(name).as_deref() {
        Ok("axum") => Ok(vec![BenchLayer::Axum]),
        Ok("service") => Ok(vec![BenchLayer::Service]),
        Ok("all") | Err(_) => Ok(vec![BenchLayer::Axum, BenchLayer::Service]),
        Ok(value) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be axum, service, or all; got {value}"),
        )),
    }
}

fn backend_name(backend: DatabaseBackend) -> &'static str {
    match backend {
        DatabaseBackend::Sled => "sled",
        DatabaseBackend::Sqlite => "sqlite",
    }
}

fn database_name(backend: DatabaseBackend) -> &'static str {
    match backend {
        DatabaseBackend::Sled => "zhorten-sled.db",
        DatabaseBackend::Sqlite => "zhorten-sqlite.db",
    }
}

#[derive(Clone, Copy)]
enum BenchLayer {
    Axum,
    Service,
}

impl BenchLayer {
    fn name(self) -> &'static str {
        match self {
            Self::Axum => "axum over TCP keep-alive",
            Self::Service => "raw zhorten_service layer",
        }
    }
}

struct BenchResult {
    elapsed: Duration,
    requests: u64,
}

impl BenchResult {
    fn requests_per_second(&self) -> f64 {
        self.requests as f64 / self.elapsed.as_secs_f64()
    }
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
