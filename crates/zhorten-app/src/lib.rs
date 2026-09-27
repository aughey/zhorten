#![recursion_limit = "512"]
#![cfg_attr(not(feature = "csr"), allow(dead_code, unused_variables))]

#[cfg(feature = "csr")]
mod api_helper;
#[cfg(feature = "csr")]
use api_helper::{api, api_nobody};
use leptos::prelude::*;
use leptos_meta::{Stylesheet, Title, provide_meta_context};
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};
use qrcode::{QrCode, render::svg};
use zhorten_core::ValidCode;
#[cfg(feature = "csr")]
use zhorten_core::api::LoginRequest;
use zhorten_core::api::{CreateRequest, DashboardData, LinkRecord};

#[component]
/// Root Leptos component with the public home page and admin console routes.
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        <Title text="zhorten — tiny links"/>
        <Stylesheet id="zhorten" href="/assets/style.css"/>
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

#[derive(PartialEq, Eq, Debug)]
enum AdminState {
    Loading,
    Login,
    Error(String),
    Dashboard { data: DashboardData },
}

#[component]
fn Admin() -> impl IntoView {
    let admin_state = RwSignal::new({
        #[cfg(feature = "csr")]
        {
            AdminState::Loading
        }
        #[cfg(not(feature = "csr"))]
        {
            AdminState::Login
        }
    });

    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        // On page load, try the protected endpoint first. A valid session skips
        // the login form; an expired or missing session lands on the login view.
        leptos::task::spawn_local(async move {
            match api_nobody::<DashboardData>("GET", "/api/links").await {
                Ok(data) => admin_state.set(AdminState::Dashboard { data }),
                Err(error) if error == "unauthorized" => admin_state.set(AdminState::Login),
                Err(error) => admin_state.set(AdminState::Error(error)),
            }
        });
    });

    let spinner = || view! { <main class="center-shell"><div class="spinner"></div></main> };
    let show_dashboard = Callback::new(move |data| {
        admin_state.set(AdminState::Dashboard { data });
    });
    let show_login = Callback::new(move |()| admin_state.set(AdminState::Login));

    view! {
        {move || match &*admin_state.read() {
            AdminState::Loading => spinner().into_any(),
            AdminState::Login => view! { <Login on_login=show_dashboard/> }.into_any(),
            AdminState::Error(error_message) => view! {
                <main class="center-shell">
                    <div class="error-banner">{error_message.clone()}</div>
                </main>
            }.into_any(),
            AdminState::Dashboard { data } => {
                let initial_data = data.clone();
                view! { <Dashboard initial_data on_logout=show_login/> }.into_any()
            },
        }}
    }
}

#[component]
fn Login(on_login: Callback<DashboardData>) -> impl IntoView {
    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(None::<String>);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_busy.set(true);
        set_error.set(None);
        let username = username.get();
        let password = password.get();
        #[cfg(feature = "csr")]
        leptos::task::spawn_local(async move {
            match api("POST", "/api/login", LoginRequest { username, password }).await {
                Ok(data) => on_login.run(data),
                Err(error) => set_error.set(Some(error)),
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
fn Dashboard(initial_data: DashboardData, on_logout: Callback<()>) -> impl IntoView {
    let data = RwSignal::new(initial_data);
    let (error, set_error) = signal(None::<String>);

    // Keep list reloading in one closure so create/delete flows can share the
    // same state transition and preserve the current authenticated view.
    let refresh = Callback::new(move |()| {
        #[cfg(feature = "csr")]
        leptos::task::spawn_local(async move {
            match api_nobody::<DashboardData>("GET", "/api/links").await {
                Ok(new_data) => {
                    data.set(new_data);
                    set_error.set(None);
                }
                Err(error) if error == "unauthorized" => on_logout.run(()),
                Err(error) => set_error.set(Some(error)),
            }
        });
    });
    let create = Action::new_local(move |request: &CreateRequest| {
        set_error.set(None);
        let request = request.clone();
        async move {
            #[cfg(feature = "csr")]
            {
                api::<LinkRecord, _>("POST", "/api/links", request).await
            }
            #[cfg(not(feature = "csr"))]
            {
                Err("browser API unavailable".to_owned())
            }
        }
    });
    let create_result = create.value();
    Effect::new(move |_| {
        if let Some(result) = create_result.get() {
            match result {
                Ok(_) => refresh.run(()),
                Err(error) if error == "unauthorized" => on_logout.run(()),
                Err(error) => set_error.set(Some(error)),
            }
        }
    });
    let delete_link = Callback::new(move |code: ValidCode| {
        #[cfg(feature = "csr")]
        leptos::task::spawn_local(async move {
            let path = format!("/api/links/{code}");
            let result = api_nobody::<serde_json::Value>("DELETE", &path).await;
            match result {
                Ok(_) => {
                    data.update(|data| {
                        let removed_clicks = data
                            .links
                            .iter()
                            .find(|link| link.code == code)
                            .map(|link| link.clicks)
                            .unwrap_or(0);
                        data.links.retain(|link| link.code != code);
                        data.total_clicks = data.total_clicks.saturating_sub(removed_clicks);
                    });
                    set_error.set(None);
                }
                Err(error) if error == "unauthorized" => on_logout.run(()),
                Err(error) => set_error.set(Some(error)),
            }
        });
    });
    let logout = move |_| {
        #[cfg(feature = "csr")]
        leptos::task::spawn_local(async move {
            let _ = api::<serde_json::Value, _>("POST", "/api/logout", serde_json::json!({})).await;
            on_logout.run(());
        });
    };

    view! {
        <div class="app-shell">
            <header><div class="brand-row"><div class="brand-mark small">"Z"</div><span>"zhorten"</span></div><button class="ghost" on:click=logout>"Sign out"</button></header>
            <main class="dashboard">
                <section class="hero-row">
                    <div><span class="eyebrow">"ADMIN CONSOLE"</span><h1>"Your links"</h1><p class="muted">"Create compact links and see where attention lands."</p></div>
                    <div class="stat"><strong>{move || data.read().total_clicks}</strong><span>"total clicks"</span></div>
                </section>
                <CreateCode create/>
                <ErrorBanner error/>
                <section class="links-section">
                    <div class="section-head"><h2>"All links"</h2><span class="pill">{move || format!("{} ACTIVE", data.read().links.len())}</span></div>
                    <div class="link-list">
                        <For each=move || data.read().links.clone() key=|item| item.code.clone() let:item>
                            <LinkRow item on_delete=delete_link/>
                        </For>
                        <Show when=move || data.read().links.is_empty()>
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
fn CreateCode(create: Action<CreateRequest, Result<LinkRecord, String>>) -> impl IntoView {
    let (code, set_code) = signal(String::new());
    let (url, set_url) = signal(String::new());
    let pending = create.pending();
    let result = create.value();

    Effect::new(move |_| {
        if matches!(&*result.read(), Some(Ok(_))) {
            set_code.set(String::new());
            set_url.set(String::new());
        }
    });

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if pending.get_untracked() {
            return;
        }
        create.dispatch(CreateRequest {
            code: code.get(),
            url: url.get(),
        });
    };

    view! {
        <section class="creator-card">
            <form on:submit=submit>
                <label class="code-field"><span>"SHORT CODE"</span><div class="input-prefix"><span>"/z/"</span><input placeholder="example_short_name" pattern="[A-Za-z0-9_\\-]+" minlength="1" maxlength="32" required disabled=move || pending.get() prop:value=code on:input=move |e| set_code.set(event_target_value(&e))/></div></label>
                <label class="url-field"><span>"DESTINATION URL"</span><input type="url" placeholder="https://example.com/destination" required disabled=move || pending.get() prop:value=url on:input=move |e| set_url.set(event_target_value(&e))/></label>
                <button class="button" type="submit" disabled=move || pending.get()>{move || if pending.get() { "Creating…" } else { "Create link" }}</button>
            </form>
        </section>
    }
}

#[component]
fn LinkRow(item: LinkRecord, on_delete: Callback<ValidCode>) -> impl IntoView {
    let code_for_delete = item.code.clone();
    let short_path = format!("/z/{}", item.code);
    let destination = item.url.clone();
    let delete = move |_| {
        let code = code_for_delete.clone();
        #[cfg(feature = "csr")]
        {
            // Confirmation lives client-side because delete is intentionally a
            // simple API no-op for missing codes.
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
            on_delete.run(code);
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
    // Store the modal path outside the reactive closure so QR generation keeps a
    // stable value even while the row is re-rendered.
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

/// Turn an app-relative short path into a shareable URL when running in a browser.
fn absolute_short_url(path: &str) -> String {
    #[cfg(feature = "csr")]
    if let Some(window) = web_sys::window()
        && let Ok(origin) = window.location().origin()
    {
        return format!("{origin}{path}");
    }
    path.to_owned()
}

/// Render a QR code as SVG markup for direct insertion into the modal.
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

/// Format Unix timestamps for the browser UI, with a plain fallback for non-CSR builds.
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
    view! { <Show when=move || error.read().is_some()>{move || view! { <div class="error-banner">{error.get().unwrap_or_default()}</div> }}</Show> }
}

#[cfg(feature = "csr")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}
