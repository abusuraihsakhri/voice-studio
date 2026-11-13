//! Bounded, proxy-free loopback transport. Never follows redirects or executes models.
use std::ptr::{null, null_mut};
use windows_sys::Win32::Networking::WinHttp::*;

struct InternetHandle(*mut std::ffi::c_void);
impl Drop for InternetHandle {
    fn drop(&mut self) {
        unsafe {
            WinHttpCloseHandle(self.0);
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

pub struct BackendClient {
    port: u16,
    token: Option<String>,
}

impl BackendClient {
    pub fn new(port: u16, token: Option<String>) -> Result<Self, String> {
        if port == 0 {
            return Err("Enter a local backend port between 1 and 65535.".into());
        }
        if token
            .as_ref()
            .is_some_and(|t| t.contains(['\r', '\n']) || t.len() > 4096)
        {
            return Err("Invalid backend credential.".into());
        }
        Ok(Self { port, token })
    }

    pub fn get_json(&self, path: &str) -> Result<serde_json::Value, String> {
        if !matches!(path, "/health" | "/engines" | "/profiles") {
            return Err("Unsupported native-client endpoint.".into());
        }
        unsafe {
            let session = InternetHandle(WinHttpOpen(
                wide("VoiceStudioNative/0.1").as_ptr(),
                WINHTTP_ACCESS_TYPE_NO_PROXY,
                null(),
                null(),
                0,
            ));
            if session.0.is_null() {
                return Err("Cannot initialize local backend connection.".into());
            }
            if WinHttpSetTimeouts(session.0, 2000, 2000, 5000, 5000) == 0 {
                return Err("Cannot bound backend connection timeout.".into());
            }
            let connection = InternetHandle(WinHttpConnect(
                session.0,
                wide("127.0.0.1").as_ptr(),
                self.port,
                0,
            ));
            if connection.0.is_null() {
                return Err("Cannot connect to the selected local backend.".into());
            }
            let request = InternetHandle(WinHttpOpenRequest(
                connection.0,
                wide("GET").as_ptr(),
                wide(path).as_ptr(),
                null(),
                null(),
                null(),
                0,
            ));
            if request.0.is_null() {
                return Err("Cannot create backend request.".into());
            }
            let mut redirects = WINHTTP_OPTION_REDIRECT_POLICY_NEVER;
            if WinHttpSetOption(
                request.0,
                WINHTTP_OPTION_REDIRECT_POLICY,
                &mut redirects as *mut _ as _,
                std::mem::size_of_val(&redirects) as u32,
            ) == 0
            {
                return Err("Cannot enforce local-only backend policy.".into());
            }
            let headers = wide(
                &self
                    .token
                    .as_ref()
                    .map(|t| format!("Accept: application/json\r\nAuthorization: Bearer {t}\r\n"))
                    .unwrap_or_else(|| "Accept: application/json\r\n".into()),
            );
            if WinHttpSendRequest(
                request.0,
                headers.as_ptr(),
                (headers.len() - 1) as u32,
                null(),
                0,
                0,
                0,
            ) == 0
                || WinHttpReceiveResponse(request.0, null_mut()) == 0
            {
                return Err("Local backend did not respond within the connection budget. Start a compatible backend and retry.".into());
            }
            let mut status: u32 = 0;
            let mut length = 4;
            if WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                null(),
                &mut status as *mut _ as _,
                &mut length,
                null_mut(),
            ) == 0
            {
                return Err("Backend response status unavailable.".into());
            }
            if status != 200 {
                return Err(format!(
                    "Local backend returned HTTP {status}. Check its readiness and authentication."
                ));
            }
            let mut declared_length: u32 = 0;
            let mut declared_size = 4;
            if WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_CONTENT_LENGTH | WINHTTP_QUERY_FLAG_NUMBER,
                null(),
                &mut declared_length as *mut _ as _,
                &mut declared_size,
                null_mut(),
            ) != 0
                && declared_length > 256 * 1024
            {
                return Err("Backend response exceeds the native client's 256 KiB limit.".into());
            }
            let mut body = Vec::new();
            let mut chunk = [0u8; 8192];
            loop {
                let mut read = 0;
                if WinHttpReadData(
                    request.0,
                    chunk.as_mut_ptr() as _,
                    chunk.len() as u32,
                    &mut read,
                ) == 0
                {
                    return Err(format!(
                        "Backend response interrupted (Windows error {}).",
                        std::io::Error::last_os_error()
                            .raw_os_error()
                            .unwrap_or_default()
                    ));
                }
                if read == 0 {
                    break;
                }
                if body.len() + read as usize > 256 * 1024 {
                    return Err(
                        "Backend response exceeds the native client's 256 KiB limit.".into(),
                    );
                }
                body.extend_from_slice(&chunk[..read as usize]);
            }
            serde_json::from_slice(&body).map_err(|_| "Backend returned invalid JSON.".into())
        }
    }

    pub fn inventory(&self) -> Result<String, String> {
        let health = self.get_json("/health")?;
        if health.get("status").and_then(|v| v.as_str()) != Some("ok") {
            return Err(
                "Backend is reachable but is not ready (expected health status 'ok').".into(),
            );
        }
        let engines = self.get_json("/engines")?;
        let profiles = self.get_json("/profiles")?;
        let voices = profiles
            .as_array()
            .ok_or("Backend voice-library response is incompatible.")?
            .len();
        let tts = &engines["tts"];
        let count = tts
            .as_array()
            .or_else(|| tts.get("backends").and_then(|v| v.as_array()))
            .or_else(|| tts.get("engines").and_then(|v| v.as_array()))
            .map(|v| v.len());
        Ok(format!(
            "Local backend ready · {} voices · {} TTS engines. Discovery only: external backend memory is not contained by this window.",
            voices,
            count
                .map(|n| n.to_string())
                .unwrap_or_else(|| "inventory available".into())
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    fn fixture(response: String) -> (u16, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let thread = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut raw = Vec::new();
            let mut buf = [0; 1024];
            while !raw.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = socket.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                raw.extend_from_slice(&buf[..n]);
            }
            let _ = socket.write_all(response.as_bytes());
            String::from_utf8(raw).unwrap()
        });
        (port, thread)
    }
    #[test]
    fn bounded_authenticated_loopback_request() {
        let (port, server) = fixture(
            "HTTP/1.1 200 OK\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}"
                .into(),
        );
        assert_eq!(
            BackendClient::new(port, Some("test-only".into()))
                .unwrap()
                .get_json("/health")
                .unwrap()["status"],
            "ok"
        );
        let request = server.join().unwrap();
        assert!(request.contains("Authorization: Bearer test-only"));
        assert!(request.starts_with("GET /health "));
    }
    #[test]
    fn refuses_redirect_and_oversized_body() {
        let (port,server)=fixture("HTTP/1.1 302 Found\r\nLocation: http://example.invalid/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into());
        assert!(
            BackendClient::new(port, None)
                .unwrap()
                .get_json("/health")
                .unwrap_err()
                .contains("302")
        );
        server.join().unwrap();
        let body = "x".repeat(262145);
        let (port, server) = fixture(format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ));
        let error = BackendClient::new(port, None)
            .unwrap()
            .get_json("/health")
            .unwrap_err();
        assert!(error.contains("256 KiB"), "unexpected error: {error}");
        server.join().unwrap();
    }
    #[test]
    fn validates_port_credentials_and_route() {
        assert!(BackendClient::new(0, None).is_err());
        assert!(BackendClient::new(3900, Some("token\r\nInjected: yes".into())).is_err());
        assert!(
            BackendClient::new(3900, None)
                .unwrap()
                .get_json("http://example.invalid")
                .is_err()
        );
    }
}
