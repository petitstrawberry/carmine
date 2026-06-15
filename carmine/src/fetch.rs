pub fn fetch_url(url: &str) -> Result<String, String> {
    let resp = ureq::get(url)
        .set("User-Agent", "carmine/0.1")
        .set("Accept", "text/html,*/*;q=0.8")
        .call()
        .map_err(|e| format!("request failed: {}", e))?;

    resp.into_string().map_err(|e| format!("read body: {}", e))
}

pub fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
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

use std::io::Read;
