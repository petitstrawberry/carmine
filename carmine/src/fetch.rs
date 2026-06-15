use std::io::Read;
use std::sync::{Mutex, OnceLock};

pub trait HttpBackend: Send + Sync {
    fn fetch_text(&self, url: &str) -> Result<String, String>;
    fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, String>;
}

struct UreqBackend;

pub fn fetch_url(url: &str) -> Result<String, String> {
    with_backend(|backend| backend.fetch_text(url))
}

pub fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    with_backend(|backend| backend.fetch_bytes(url))
}

pub fn set_http_backend<B>(backend: B) -> Result<(), String>
where
    B: HttpBackend + 'static,
{
    let mut slot = backend_slot()
        .lock()
        .map_err(|_| "http backend lock poisoned".to_string())?;
    *slot = Box::new(backend);
    Ok(())
}

fn with_backend<T>(f: impl FnOnce(&dyn HttpBackend) -> Result<T, String>) -> Result<T, String> {
    let slot = backend_slot()
        .lock()
        .map_err(|_| "http backend lock poisoned".to_string())?;
    f(slot.as_ref())
}

fn backend_slot() -> &'static Mutex<Box<dyn HttpBackend>> {
    static BACKEND: OnceLock<Mutex<Box<dyn HttpBackend>>> = OnceLock::new();
    BACKEND.get_or_init(|| Mutex::new(Box::new(UreqBackend)))
}

impl HttpBackend for UreqBackend {
    fn fetch_text(&self, url: &str) -> Result<String, String> {
    let resp = ureq::get(url)
        .set("User-Agent", "carmine/0.1")
        .set("Accept", "text/html,*/*;q=0.8")
        .call()
        .map_err(|e| format!("request failed: {}", e))?;

    resp.into_string().map_err(|e| format!("read body: {}", e))
    }

    fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let resp = ureq::get(url)
            .set("User-Agent", "carmine/0.1")
            .set("Accept", "*/*")
            .call()
            .map_err(|e| format!("request failed: {}", e))?;

        let mut buf = Vec::new();
        resp.into_reader()
            .read_to_end(&mut buf)
            .map_err(|e| format!("read body: {}", e))?;
        Ok(buf)
    }
}
