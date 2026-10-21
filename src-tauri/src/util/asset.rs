//! URLs the webview can load local files from (Tauri's asset protocol).
//!
//! Atlas stores local images as "file:<absolute path>" and turns them into asset URLs
//! when building DTOs, so the UI just sees a URL. The protocol only serves paths inside
//! the scope in tauri.conf.json (`app.security.assetProtocol`): the icon cache.

/// "file:C:\x\a.png" → "http://asset.localhost/C%3A%5Cx%5Ca.png" (what Tauri's
/// `convertFileSrc` makes on Windows). Anything else (http URLs, keys) passes through.
pub fn url(stored: &str) -> String {
    match stored.strip_prefix("file:") {
        Some(path) => format!("http://asset.localhost/{}", encode_component(path)),
        None => stored.to_string(),
    }
}

/// Like JavaScript's encodeURIComponent.
fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn matches_convert_file_src() {
        assert_eq!(
            super::url(r"file:C:\Users\me\AppData\Roaming\dev.keplerhaloxx.atlas\cache\icons\ab.png"),
            "http://asset.localhost/C%3A%5CUsers%5Cme%5CAppData%5CRoaming%5Cdev.keplerhaloxx.atlas%5Ccache%5Cicons%5Cab.png"
        );
        assert_eq!(super::url("file:C:\\a b\\é.png"), "http://asset.localhost/C%3A%5Ca%20b%5C%C3%A9.png");
        assert_eq!(super::url("https://cdn.modrinth.com/x.png"), "https://cdn.modrinth.com/x.png");
    }
}
