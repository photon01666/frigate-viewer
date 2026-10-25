//! The small, dependency-free configuration pages served to the WebView.
//!
//! These pages intentionally contain no JavaScript. They are only ever served
//! from a loopback-bound HTTP server with a random path token.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageMode {
    Setup,
    Settings,
}

pub struct PageData {
    pub mode: PageMode,
    pub base_path: String,
    pub url: String,
    pub allow_invalid_certificates: bool,
    pub keep_screen_on: bool,
    pub error: Option<String>,
    pub force_available: bool,
    pub offline: bool,
    pub current_server: Option<String>,
}

pub fn render(data: &PageData) -> String {
    let is_settings = data.mode == PageMode::Settings;
    let page_title = if is_settings { "Server settings" } else { "Connect to your NVR" };
    let lede = if is_settings {
        "Update the address of your Frigate server or change how the viewer behaves."
    } else {
        "Enter the address you use to open Frigate in a browser. Live view, recordings and events all run inside the app."
    };
    let submit_label = if is_settings { "Save &amp; reconnect" } else { "Connect to Frigate" };

    let icon = format!("{}icon.png", data.base_path);
    let save_action = format!("{}save", data.base_path);
    let forget_action = format!("{}forget", data.base_path);

    let offline_notice = if data.offline {
        let address = data
            .current_server
            .as_deref()
            .map(escape)
            .unwrap_or_else(|| "your Frigate server".to_string());
        format!(
            r#"<div class="notice error" role="alert"><strong>Could not reach {address}.</strong><br>Check that Frigate is online and that this phone is on the same network, then try again below.</div>"#
        )
    } else {
        String::new()
    };

    let error_notice = match &data.error {
        Some(error) => format!(
            r#"<div class="notice error" role="alert">{}</div>"#,
            escape(error)
        ),
        None => String::new(),
    };

    let force_button = if data.force_available {
        r#"<button class="secondary" type="submit" name="force" value="1">Save anyway</button>"#
    } else {
        ""
    };

    let back_link = if is_settings {
        match &data.current_server {
            Some(server) => format!(
                r#"<a class="secondary-link" href="{}">Back to Frigate</a>"#,
                escape_attr(server)
            ),
            None => String::new(),
        }
    } else {
        String::new()
    };

    let forget_section = if is_settings && data.current_server.is_some() {
        format!(
            r#"
            <div class="divider"></div>
            <form class="forget-form" method="post" action="{forget_action}">
                <div class="small-actions">
                    <button class="forget" type="submit">Forget this server</button>
                </div>
            </form>"#
        )
    } else {
        String::new()
    };

    format!(
        r##"<!doctype html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
    <meta name="color-scheme" content="dark">
    <meta name="theme-color" content="#0A0E13">
    <meta name="format-detection" content="telephone=no">
    <title>Frigate Viewer · {page_title}</title>
    <link rel="icon" href="{icon}" type="image/png">
    <style>
        :root {{
            color-scheme: dark;
            --bg: #0a0e13;
            --ink: #f3f1ea;
            --muted: #98a2b3;
            --line: rgba(243, 241, 234, .11);
            --line-strong: rgba(243, 241, 234, .22);
            --danger: #fecdd3;
            --danger-bg: rgba(190, 18, 60, .13);
            --danger-line: rgba(244, 63, 94, .34);
        }}
        * {{ box-sizing: border-box; }}
        html, body {{ margin: 0; min-height: 100%; }}
        body {{
            display: flex;
            min-height: 100dvh;
            background: var(--bg);
            color: var(--ink);
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            -webkit-font-smoothing: antialiased;
            overflow-x: hidden;
        }}
        body::before {{
            content: "";
            position: fixed;
            inset: 0;
            pointer-events: none;
            background:
                radial-gradient(ellipse 85% 55% at 50% -12%, rgba(243, 241, 234, .12), transparent 64%),
                radial-gradient(ellipse 65% 52% at 112% 112%, rgba(96, 130, 160, .10), transparent 70%);
        }}
        .shell {{
            position: relative;
            margin: auto;
            width: 100%;
            max-width: 500px;
            padding: 44px 20px calc(30px + env(safe-area-inset-bottom));
        }}
        .brand {{ display: flex; align-items: center; gap: 12px; }}
        .brand img {{
            width: 46px; height: 46px; border-radius: 13px;
            box-shadow: 0 8px 24px rgba(0, 0, 0, .38);
        }}
        .brand .name {{
            display: block;
            font-size: 11.5px; font-weight: 700; letter-spacing: .18em;
            color: #d8d4cb;
        }}
        .brand .sub {{
            display: block; margin-top: 3px;
            font-size: 10px; font-weight: 500; letter-spacing: .06em;
            color: #7c8798;
        }}
        h1 {{
            margin: 26px 0 12px;
            font-size: clamp(30px, 8vw, 36px);
            font-weight: 650;
            line-height: 1.08;
            letter-spacing: -.035em;
        }}
        .lede {{
            margin: 0 0 26px;
            max-width: 42ch;
            color: var(--muted);
            font-size: 15px;
            line-height: 1.6;
        }}
        .card {{
            padding: 22px;
            border: 1px solid var(--line);
            border-radius: 20px;
            background: linear-gradient(180deg, rgba(255, 255, 255, .045), rgba(255, 255, 255, .018));
            box-shadow: 0 24px 80px rgba(0, 0, 0, .45);
            -webkit-backdrop-filter: blur(18px);
            backdrop-filter: blur(18px);
        }}
        .field-label {{
            display: block;
            margin-bottom: 9px;
            color: #c8c3b9;
            font-size: 12px;
            font-weight: 700;
            letter-spacing: .08em;
            text-transform: uppercase;
        }}
        input[type="text"] {{
            width: 100%;
            height: 52px;
            padding: 0 14px;
            border: 1px solid var(--line-strong);
            border-radius: 12px;
            outline: none;
            background: #0c1219;
            color: var(--ink);
            font: inherit;
            font-size: 16px;
        }}
        input[type="text"]::placeholder {{ color: #5f6b7a; }}
        input[type="text"]:focus {{
            border-color: rgba(243, 241, 234, .55);
            box-shadow: 0 0 0 3px rgba(243, 241, 234, .10);
        }}
        .hint {{
            margin: 9px 0 0;
            color: #7c8798;
            font-size: 12.5px;
            line-height: 1.5;
        }}
        .options {{ display: grid; gap: 14px; margin-top: 20px; }}
        .option {{
            display: flex;
            gap: 11px;
            align-items: flex-start;
            color: #d8d4cb;
            cursor: pointer;
            font-size: 14px;
            line-height: 1.4;
        }}
        .option input {{
            -webkit-appearance: none;
            appearance: none;
            display: grid;
            place-content: center;
            flex: 0 0 auto;
            width: 20px;
            height: 20px;
            margin: 1px 0 0;
            border: 1.5px solid var(--line-strong);
            border-radius: 6px;
            background: #0c1219;
        }}
        .option input:checked {{
            border-color: var(--ink);
            background: var(--ink);
        }}
        .option input:checked::after {{
            content: "";
            width: 11px;
            height: 6px;
            border-left: 2px solid #0b0f14;
            border-bottom: 2px solid #0b0f14;
            transform: rotate(-45deg) translate(1px, -1px);
        }}
        .option small {{
            display: block;
            margin-top: 3px;
            color: #7c8798;
            font-size: 12.5px;
            line-height: 1.45;
        }}
        button, .secondary-link {{
            display: flex;
            align-items: center;
            justify-content: center;
            width: 100%;
            min-height: 50px;
            border-radius: 12px;
            font: inherit;
            font-size: 15.5px;
            font-weight: 700;
            text-decoration: none;
            cursor: pointer;
        }}
        .primary {{
            margin-top: 22px;
            border: 1px solid transparent;
            background: var(--ink);
            color: #0a0e13;
        }}
        .primary:active {{ transform: translateY(1px); background: #e5e1d8; }}
        .secondary {{
            margin-top: 10px;
            border: 1px solid var(--line-strong);
            background: transparent;
            color: var(--ink);
        }}
        .secondary:active {{ background: rgba(255, 255, 255, .06); }}
        .secondary-link {{
            min-height: 46px;
            margin-top: 10px;
            border: 1px solid transparent;
            color: #cfcac1;
            font-size: 14.5px;
            font-weight: 600;
        }}
        .secondary-link:hover {{ border-color: var(--line); }}
        .notice {{
            margin-bottom: 18px;
            padding: 13px 14px;
            border-radius: 13px;
            font-size: 13.5px;
            line-height: 1.55;
        }}
        .notice.error {{
            border: 1px solid var(--danger-line);
            background: var(--danger-bg);
            color: var(--danger);
        }}
        .divider {{ height: 1px; margin: 22px 0 6px; background: var(--line); }}
        .small-actions {{ display: flex; justify-content: center; margin-top: 6px; }}
        .forget {{
            width: auto;
            min-height: 44px;
            padding: 0 12px;
            border: 0;
            background: none;
            color: #8a93a3;
            font-size: 12.5px;
            font-weight: 600;
        }}
        .forget:hover {{ color: #fda4af; }}
        footer {{
            margin-top: 26px;
            padding: 0 12px;
            color: #6b7686;
            font-size: 11.5px;
            line-height: 1.6;
            text-align: center;
        }}
        @media (max-width: 380px) {{
            .card {{ padding: 18px; }}
            h1 {{ font-size: 29px; }}
        }}
        @media (max-height: 650px) {{
            .shell {{ padding-top: 24px; }}
            h1 {{ margin-top: 18px; }}
        }}
    </style>
</head>
<body>
    <main class="shell">
        <header>
            <div class="brand">
                <img src="{icon}" alt="Frigate">
                <div>
                    <span class="name">FRIGATE VIEWER</span>
                    <span class="sub">NATIVE WEBVIEW SHELL</span>
                </div>
            </div>
            <h1>{page_title}</h1>
            <p class="lede">{lede}</p>
        </header>

        <section class="card">
            {offline_notice}
            {error_notice}
            <form method="post" action="{save_action}">
                <label class="field-label" for="server-url">Frigate server URL</label>
                <input
                    id="server-url"
                    name="url"
                    type="text"
                    inputmode="url"
                    enterkeyhint="go"
                    autocomplete="url"
                    autocapitalize="none"
                    autocorrect="off"
                    spellcheck="false"
                    required
                    value="{url}"
                    placeholder="http://192.168.1.10:5000">

                <p class="hint">Examples: <strong>http://192.168.1.10:5000</strong> for the local UI,
                or <strong>https://your-frigate:8971</strong> when authentication is enabled.</p>

                <div class="options">
                    <label class="option">
                        <input type="checkbox" name="allow_invalid" value="1" {allow_invalid_checked}>
                        <span>
                            Allow self-signed certificates
                            <small>Only use this with a certificate you trust on your own network.</small>
                        </span>
                    </label>
                    <label class="option">
                        <input type="checkbox" name="keep_screen_on" value="1" {keep_screen_checked}>
                        <span>Keep the screen awake while viewing</span>
                    </label>
                </div>

                <button class="primary" type="submit">{submit_label}</button>
                {back_link}
                {force_button}
            </form>
            {forget_section}
        </section>

        <footer>
            Frigate Viewer v{version} · Independent viewer for Frigate NVR.<br>
            Not affiliated with the Frigate project. Frigate bird mark by the Frigate contributors.
        </footer>
    </main>
</body>
</html>"##,
        version = env!("CARGO_PKG_VERSION"),
        icon = icon,
        page_title = page_title,
        lede = lede,
        offline_notice = offline_notice,
        error_notice = error_notice,
        save_action = save_action,
        url = escape_attr(&data.url),
        allow_invalid_checked = checked(data.allow_invalid_certificates),
        keep_screen_checked = checked(data.keep_screen_on),
        submit_label = submit_label,
        back_link = back_link,
        force_button = force_button,
        forget_section = forget_section,
    )
}

fn checked(value: bool) -> &'static str {
    if value {
        "checked"
    } else {
        ""
    }
}

fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_attr(input: &str) -> String {
    escape(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PageData {
        PageData {
            mode: PageMode::Setup,
            base_path: "/abc123/".to_string(),
            url: "http://192.168.1.2:5000/".to_string(),
            allow_invalid_certificates: false,
            keep_screen_on: true,
            error: None,
            force_available: false,
            offline: false,
            current_server: None,
        }
    }

    #[test]
    fn renders_form_without_js() {
        let html = render(&sample());
        assert!(html.contains("http://192.168.1.2:5000/"));
        assert!(!html.contains("<script"));
        assert!(html.contains("action=\"/abc123/save\""));
    }

    #[test]
    fn escapes_untrusted_text() {
        let mut data = sample();
        data.url = "\"><script>alert(1)</script>".to_string();
        data.error = Some("<b>bad</b>".to_string());
        let html = render(&data);
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;b&gt;bad&lt;/b&gt;"));
    }
}
