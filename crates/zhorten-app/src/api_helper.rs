use serde::{Deserialize, Serialize};
use zhorten_core::api::ApiError;

#[cfg(feature = "csr")]
/// Issue a same-origin JSON API request from the browser bundle.
///
/// A 401 is normalized to the sentinel string `"unauthorized"` because the UI
/// uses that response to switch between the login and dashboard screens.
pub async fn api<T: for<'de> Deserialize<'de>, B: Serialize>(
    method: &str,
    path: &str,
    body: B,
) -> Result<T, String> {
    let json = serde_json::to_string(&body).map_err(|e| e.to_string())?;
    api_raw(method, path, Some(json)).await
}

#[cfg(feature = "csr")]
/// Like [`api`], but for requests that send no body.
pub async fn api_nobody<T: for<'de> Deserialize<'de>>(
    method: &str,
    path: &str,
) -> Result<T, String> {
    api_raw(method, path, None).await
}

#[cfg(feature = "csr")]
async fn api_raw<T: for<'de> Deserialize<'de>>(
    method: &str,
    path: &str,
    body: Option<String>,
) -> Result<T, String> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, RequestMode, Response};

    let opts = RequestInit::new();
    opts.set_method(method);
    opts.set_mode(RequestMode::SameOrigin);
    if let Some(body) = body {
        opts.set_body(&wasm_bindgen::JsValue::from_str(&body));
    }
    let request = Request::new_with_str_and_init(path, &opts).map_err(|e| format!("{e:?}"))?;
    request
        .headers()
        .set("Content-Type", "application/json")
        .map_err(|e| format!("{e:?}"))?;
    let window = web_sys::window().ok_or("browser window unavailable")?;
    let response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("{e:?}"))?;
    let response: Response = response.dyn_into().map_err(|_| "invalid response")?;
    let status = response.status();
    let text = JsFuture::from(response.text().map_err(|e| format!("{e:?}"))?)
        .await
        .map_err(|e| format!("{e:?}"))?
        .as_string()
        .unwrap_or_default();
    if status == 401 {
        return Err("unauthorized".into());
    }
    if !response.ok() {
        let message = serde_json::from_str::<ApiError>(&text)
            .map(|e| e.error)
            .unwrap_or(text);
        return Err(message);
    }
    serde_json::from_str(&text).map_err(|e| e.to_string())
}
