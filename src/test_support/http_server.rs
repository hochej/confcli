use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use url::Url;

pub struct TestServer {
    pub base_url: String,
    pub shutdown: oneshot::Sender<()>,
    pub hits: Arc<AtomicUsize>,
}

impl TestServer {
    pub fn url_string(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    pub fn url(&self, path: &str) -> Url {
        Url::parse(&self.url_string(path)).unwrap()
    }
}

pub async fn start_server<F>(handler: F) -> TestServer
where
    F: Fn(usize, &str) -> (u16, Vec<(String, String)>, Vec<u8>) + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    let hits = Arc::new(AtomicUsize::new(0));
    let hits_task = hits.clone();

    let (tx, mut rx) = oneshot::channel::<()>();
    let handler = Arc::new(handler);

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut rx => return,
                res = listener.accept() => {
                    let (mut sock, _) = match res {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let mut buf = vec![0u8; 8192];
                    let n = match sock.read(&mut buf).await {
                        Ok(n) => n,
                        Err(_) => continue,
                    };
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let first = req.lines().next().unwrap_or_default();
                    let raw_target = first.split_whitespace().nth(1).unwrap_or("/");
                    let target = if raw_target.starts_with("http://") || raw_target.starts_with("https://") {
                        Url::parse(raw_target).ok().map(|u| {
                            let mut s = u.path().to_string();
                            if let Some(q) = u.query() {
                                s.push('?');
                                s.push_str(q);
                            }
                            s
                        }).unwrap_or_else(|| "/".to_string())
                    } else {
                        raw_target.to_string()
                    };

                    let hit = hits_task.fetch_add(1, Ordering::SeqCst) + 1;
                    let (status, headers, body) = handler(hit, &target);

                    let reason = match status {
                        200 => "OK",
                        400 => "Bad Request",
                        404 => "Not Found",
                        429 => "Too Many Requests",
                        500 => "Internal Server Error",
                        _ => "OK",
                    };

                    let mut resp = Vec::new();
                    resp.extend_from_slice(format!("HTTP/1.1 {} {}\r\n", status, reason).as_bytes());
                    resp.extend_from_slice(b"Connection: close\r\n");
                    for (k, v) in headers {
                        resp.extend_from_slice(format!("{}: {}\r\n", k, v).as_bytes());
                    }
                    resp.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
                    resp.extend_from_slice(&body);
                    let _ = sock.write_all(&resp).await;
                    let _ = sock.shutdown().await;
                }
            }
        }
    });

    TestServer {
        base_url,
        shutdown: tx,
        hits,
    }
}
