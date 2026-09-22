#![recursion_limit = "512"]
#![cfg_attr(not(feature = "csr"), allow(dead_code, unused_variables))]

use leptos::prelude::*;
use leptos_meta::{Stylesheet, Title, provide_meta_context};
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};
use qrcode::{QrCode, render::svg};
use serde::{Deserialize, Serialize};
use zhorten_core::{DashboardData, LinkRecord};

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ApiError {
    error: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CreateRequest {
    code: String,
    url: String,
}

#[cfg(feature = "csr")]
async fn api<T: for<'de> Deserialize<'de>>(
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

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        <Title text="zhorten — tiny links"/>
        <Stylesheet id="zhorten" href="/style.css"/>
        <Router>
            <Routes fallback=|| view! { <NotFound/> }>
                <Route path=path!("/") view=Home/>
                <Route path=path!("/admin") view=Admin/>
            </Routes>
        </Router>
    }
}

#[component]
fn Home() -> impl IntoView {
    view! {
        <main class="center-shell">
            <div class="brand-mark">"Z"</div>
            <h1>"Small links. Nothing more."</h1>
            <p class="muted">"A private, self-hosted URL shortener."</p>
            <a class="text-link" href="/admin">"Administrator sign in →"</a>
        </main>
    }
}

#[component]
fn NotFound() -> impl IntoView {
    view! {
        <main class="center-shell">
            <span class="eyebrow">"404"</span>
            <h1>"That link is nowhere to be found."</h1>
            <a class="button secondary" href="/">"Back home"</a>
        </main>
    }
}

#[component]
fn Admin() -> impl IntoView {
    let (authenticated, set_authenticated) = signal(false);
    let (loading, set_loading) = signal(true);
    let (data, set_data) = signal(None::<DashboardData>);
    let (error, set_error) = signal(None::<String>);

    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            match api::<DashboardData>("GET", "/api/links", None).await {
                Ok(value) => {
                    set_data.set(Some(value));
                    set_authenticated.set(true);
                }
                Err(e) if e == "unauthorized" => set_authenticated.set(false),
                Err(e) => set_error.set(Some(e)),
            }
            set_loading.set(false);
        });
    });
    #[cfg(not(feature = "csr"))]
    set_loading.set(false);

    view! {
        <Show when=move || !loading.get() fallback=|| view! { <main class="center-shell"><div class="spinner"></div></main> }>
            <Show when=move || authenticated.get() fallback=move || view! { <Login set_authenticated set_data error set_error/> }>
                <Dashboard data set_data set_authenticated error set_error/>
            </Show>
        </Show>
    }
}

#[component]
fn Login(
    set_authenticated: WriteSignal<bool>,
    set_data: WriteSignal<Option<DashboardData>>,
    error: ReadSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
) -> impl IntoView {
    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (busy, set_busy) = signal(false);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_busy.set(true);
        set_error.set(None);
        #[cfg(feature = "csr")]
        leptos::task::spawn_local(async move {
            let body = serde_json::to_string(&LoginRequest {
                username: username.get(),
                password: password.get(),
            })
            .unwrap();
            match api::<DashboardData>("POST", "/api/login", Some(body)).await {
                Ok(value) => {
                    set_data.set(Some(value));
                    set_authenticated.set(true);
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_busy.set(false);
        });
    };

    view! {
        <main class="login-shell">
            <section class="login-card">
                <div class="brand-row"><div class="brand-mark small">"Z"</div><span>"zhorten"</span></div>
                <div>
                    <span class="eyebrow">"PRIVATE CONSOLE"</span>
                    <h1>"Welcome back."</h1>
                    <p class="muted">"Sign in to manage your short links."</p>
                </div>
                <form on:submit=submit>
                    <label>"Username"<input autocomplete="username" required prop:value=username on:input=move |e| set_username.set(event_target_value(&e))/></label>
                    <label>"Password"<input type="password" autocomplete="current-password" required prop:value=password on:input=move |e| set_password.set(event_target_value(&e))/></label>
                    <button class="button" type="submit" disabled=move || busy.get()>{move || if busy.get() { "Signing in…" } else { "Sign in" }}</button>
                </form>
                <ErrorBanner error/>
            </section>
        </main>
    }
}

#[component]
fn Dashboard(
    data: ReadSignal<Option<DashboardData>>,
    set_data: WriteSignal<Option<DashboardData>>,
    set_authenticated: WriteSignal<bool>,
    error: ReadSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
) -> impl IntoView {
    let (code, set_code) = signal(String::new());
    let (url, set_url) = signal(String::new());
    let (busy, set_busy) = signal(false);

    let refresh = move || {
        #[cfg(feature = "csr")]
        leptos::task::spawn_local(async move {
            match api::<DashboardData>("GET", "/api/links", None).await {
                Ok(value) => set_data.set(Some(value)),
                Err(e) => set_error.set(Some(e)),
            }
        });
    };
    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_busy.set(true);
        set_error.set(None);
        #[cfg(feature = "csr")]
        leptos::task::spawn_local(async move {
            let body = serde_json::to_string(&CreateRequest {
                code: code.get(),
                url: url.get(),
            })
            .unwrap();
            match api::<LinkRecord>("POST", "/api/links", Some(body)).await {
                Ok(_) => {
                    set_code.set(String::new());
                    set_url.set(String::new());
                    refresh();
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_busy.set(false);
        });
    };
    let logout = move |_| {
        #[cfg(feature = "csr")]
        leptos::task::spawn_local(async move {
            let _ = api::<serde_json::Value>("POST", "/api/logout", Some("{}".into())).await;
            set_authenticated.set(false);
            set_data.set(None);
        });
    };

    view! {
        <div class="app-shell">
            <header><div class="brand-row"><div class="brand-mark small">"Z"</div><span>"zhorten"</span></div><button class="ghost" on:click=logout>"Sign out"</button></header>
            <main class="dashboard">
                <section class="hero-row">
                    <div><span class="eyebrow">"ADMIN CONSOLE"</span><h1>"Your links"</h1><p class="muted">"Create compact links and see where attention lands."</p></div>
                    <div class="stat"><strong>{move || data.get().map(|d| d.total_clicks).unwrap_or(0)}</strong><span>"total clicks"</span></div>
                </section>
                <section class="creator-card">
                    <form on:submit=create>
                        <label class="code-field"><span>"SHORT CODE"</span><div class="input-prefix"><span>"/z/"</span><input placeholder="example_short_name" pattern="[A-Za-z0-9_-]+" minlength="1" maxlength="32" required prop:value=code on:input=move |e| set_code.set(event_target_value(&e))/></div></label>
                        <label class="url-field"><span>"DESTINATION URL"</span><input type="url" placeholder="https://example.com/destination" required prop:value=url on:input=move |e| set_url.set(event_target_value(&e))/></label>
                        <button class="button" type="submit" disabled=move || busy.get()>{move || if busy.get() { "Creating…" } else { "Create link" }}</button>
                    </form>
                </section>
                <ErrorBanner error/>
                <section class="links-section">
                    <div class="section-head"><h2>"All links"</h2><span class="pill">{move || format!("{} ACTIVE", data.get().map(|d| d.links.len()).unwrap_or(0))}</span></div>
                    <div class="link-list">
                        <For each=move || data.get().map(|d| d.links).unwrap_or_default() key=|item| item.code.clone() let:item>
                            <LinkRow item set_data set_error/>
                        </For>
                        <Show when=move || data.get().map(|d| d.links.is_empty()).unwrap_or(true)>
                            <div class="empty"><span>"↗"</span><h3>"No short links yet"</h3><p>"Create your first one above."</p></div>
                        </Show>
                    </div>
                </section>
            </main>
            <footer>"LOCAL-FIRST · SLED DATABASE"</footer>
        </div>
    }
}

#[component]
fn LinkRow(
    item: LinkRecord,
    set_data: WriteSignal<Option<DashboardData>>,
    set_error: WriteSignal<Option<String>>,
) -> impl IntoView {
    let code_for_delete = item.code.clone();
    let short_path = format!("/z/{}", item.code);
    let destination = item.url.clone();
    let delete = move |_| {
        let code = code_for_delete.clone();
        #[cfg(feature = "csr")]
        {
            let confirmed = web_sys::window()
                .and_then(|window| {
                    window
                        .confirm_with_message(&format!("Delete /z/{code}? This cannot be undone."))
                        .ok()
                })
                .unwrap_or(false);
            if !confirmed {
                return;
            }
            leptos::task::spawn_local(async move {
                let path = format!("/api/links/{code}");
                match api::<serde_json::Value>("DELETE", &path, None).await {
                    Ok(_) => set_data.update(|state| {
                        if let Some(d) = state {
                            let removed_clicks = d
                                .links
                                .iter()
                                .find(|link| link.code == code)
                                .map(|link| link.clicks)
                                .unwrap_or(0);
                            d.links.retain(|l| l.code != code);
                            d.total_clicks = d.total_clicks.saturating_sub(removed_clicks);
                        }
                    }),
                    Err(e) => set_error.set(Some(e)),
                }
            });
        }
    };
    view! {
        <article class="link-row">
            <div class="link-main"><a class="short-link" href=short_path.clone() target="_blank" rel="noopener noreferrer">{short_path.clone()}<span>"↗"</span></a><a class="destination" href=destination.clone() target="_blank" rel="noopener noreferrer">{destination.clone()}</a></div>
            <div class="click-count"><strong>{item.clicks}</strong><span>"clicks"</span></div>
            <div class="created"><span>"CREATED"</span><time>{format_date(item.created_at)}</time></div>
            <div class="row-actions">
                <LinkTools short_path=short_path.clone()/>
                <button class="icon-button danger" title="Delete link" aria-label="Delete link" on:click=delete>"×"</button>
            </div>
        </article>
    }
}

#[component]
fn LinkTools(short_path: String) -> impl IntoView {
    let (show_qr, set_show_qr) = signal(false);
    let (copied, set_copied) = signal(false);
    let copy_button_path = short_path.clone();
    let modal_path = StoredValue::new(short_path.clone());

    view! {
        <button class="action-button" title="Copy short URL" on:click=move |_| copy_short_url(&copy_button_path, set_copied)>{move || if copied.get() { "Copied" } else { "Copy" }}</button>
        <button class="action-button" title="Show QR code" on:click=move |_| set_show_qr.set(true)>"QR"</button>
        <Show when=move || show_qr.get()>
            <div class="modal-backdrop" role="presentation">
                <section class="qr-modal" role="dialog" aria-modal="true" aria-label="QR code for short link">
                    <div class="modal-head">
                        <div><span class="eyebrow">"SCAN TO TEST"</span><h3>{move || modal_path.get_value()}</h3></div>
                        <button class="icon-button" aria-label="Close QR code" on:click=move |_| set_show_qr.set(false)>"×"</button>
                    </div>
                    <div class="qr-code" inner_html=move || qr_svg(&absolute_short_url(&modal_path.get_value()))></div>
                    <p class="qr-url">{move || absolute_short_url(&modal_path.get_value())}</p>
                    <div class="modal-actions">
                        <button class="button" on:click=move |_| copy_short_url(&modal_path.get_value(), set_copied)>{move || if copied.get() { "Copied to clipboard" } else { "Copy short URL" }}</button>
                        <a class="button secondary-button" href=move || modal_path.get_value() target="_blank" rel="noopener noreferrer">"Open redirect ↗"</a>
                    </div>
                </section>
            </div>
        </Show>
    }
}

fn copy_short_url(path: &str, set_copied: WriteSignal<bool>) {
    let url = absolute_short_url(path);
    #[cfg(feature = "csr")]
    if let Some(window) = web_sys::window() {
        let promise = window.navigator().clipboard().write_text(&url);
        leptos::task::spawn_local(async move {
            if wasm_bindgen_futures::JsFuture::from(promise).await.is_ok() {
                set_copied.set(true);
            }
        });
    }
}

fn absolute_short_url(path: &str) -> String {
    #[cfg(feature = "csr")]
    if let Some(window) = web_sys::window()
        && let Ok(origin) = window.location().origin()
    {
        return format!("{origin}{path}");
    }
    path.to_owned()
}

fn qr_svg(value: &str) -> String {
    QrCode::new(value.as_bytes())
        .map(|code| {
            code.render::<svg::Color>()
                .min_dimensions(280, 280)
                .dark_color(svg::Color("#19221f"))
                .light_color(svg::Color("#ffffff"))
                .build()
        })
        .unwrap_or_else(|_| "<p>Unable to generate QR code.</p>".into())
}

fn format_date(timestamp: i64) -> String {
    #[cfg(feature = "csr")]
    {
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp as f64 * 1000.0));
        date.to_locale_date_string("en-US", &wasm_bindgen::JsValue::UNDEFINED)
            .as_string()
            .unwrap_or_default()
    }
    #[cfg(not(feature = "csr"))]
    timestamp.to_string()
}

#[component]
fn ErrorBanner(error: ReadSignal<Option<String>>) -> impl IntoView {
    view! { <Show when=move || error.get().is_some()>{move || view! { <div class="error-banner">{error.get().unwrap_or_default()}</div> }}</Show> }
}

#[cfg(feature = "csr")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}
