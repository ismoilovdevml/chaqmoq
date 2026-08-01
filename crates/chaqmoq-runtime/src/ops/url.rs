//! WHATWG URL parsing, delegated to the battle-tested `url` crate so that the
//! JS `URL` class does not have to reimplement the spec.

use deno_core::op2;
use deno_error::JsErrorBox;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlParts {
    pub href: String,
    pub origin: String,
    pub protocol: String,
    pub username: String,
    pub password: String,
    pub host: String,
    pub hostname: String,
    pub port: String,
    pub pathname: String,
    pub search: String,
    pub hash: String,
}

fn to_parts(url: &url::Url) -> UrlParts {
    UrlParts {
        href: url.to_string(),
        origin: url.origin().ascii_serialization(),
        protocol: format!("{}:", url.scheme()),
        username: url.username().to_string(),
        password: url.password().unwrap_or_default().to_string(),
        host: match url.port() {
            Some(port) => format!("{}:{port}", url.host_str().unwrap_or_default()),
            None => url.host_str().unwrap_or_default().to_string(),
        },
        hostname: url.host_str().unwrap_or_default().to_string(),
        port: url.port().map(|p| p.to_string()).unwrap_or_default(),
        pathname: url.path().to_string(),
        search: match url.query() {
            Some(q) if !q.is_empty() => format!("?{q}"),
            _ => String::new(),
        },
        hash: match url.fragment() {
            Some(f) if !f.is_empty() => format!("#{f}"),
            _ => String::new(),
        },
    }
}

#[op2]
#[serde]
pub fn op_chq_url_parse(
    #[string] href: String,
    #[string] base: Option<String>,
) -> Result<UrlParts, JsErrorBox> {
    let parsed = match base {
        Some(base) if !base.is_empty() => {
            let base = url::Url::parse(&base)
                .map_err(|e| JsErrorBox::type_error(format!("Invalid base URL: {base} ({e})")))?;
            base.join(&href)
        }
        _ => url::Url::parse(&href),
    }
    .map_err(|e| JsErrorBox::type_error(format!("Invalid URL: {href} ({e})")))?;

    Ok(to_parts(&parsed))
}

/// Applies a single mutation (`url.pathname = "..."`) and returns the whole
/// re-serialised URL, keeping normalisation rules in one place.
#[op2]
#[serde]
pub fn op_chq_url_set(
    #[string] href: String,
    #[string] property: String,
    #[string] value: String,
) -> Result<UrlParts, JsErrorBox> {
    let mut url = url::Url::parse(&href)
        .map_err(|e| JsErrorBox::type_error(format!("Invalid URL: {href} ({e})")))?;

    let bad = |what: &str| JsErrorBox::type_error(format!("Invalid {what}: {value}"));

    match property.as_str() {
        "protocol" => url
            .set_scheme(value.trim_end_matches(':'))
            .map_err(|_| bad("protocol"))?,
        "username" => url.set_username(&value).map_err(|_| bad("username"))?,
        "password" => url
            .set_password(if value.is_empty() { None } else { Some(&value) })
            .map_err(|_| bad("password"))?,
        "hostname" => url.set_host(Some(&value)).map_err(|_| bad("hostname"))?,
        "port" => {
            let port = if value.is_empty() {
                None
            } else {
                Some(value.parse::<u16>().map_err(|_| bad("port"))?)
            };
            url.set_port(port).map_err(|_| bad("port"))?
        }
        "pathname" => url.set_path(&value),
        "search" => {
            let q = value.trim_start_matches('?');
            url.set_query(if q.is_empty() { None } else { Some(q) })
        }
        "hash" => {
            let h = value.trim_start_matches('#');
            url.set_fragment(if h.is_empty() { None } else { Some(h) })
        }
        other => {
            return Err(JsErrorBox::type_error(format!(
                "Cannot set unknown URL property: {other}"
            )))
        }
    }

    Ok(to_parts(&url))
}

/// Parses `a=1&b=2` into pairs for `URLSearchParams`.
#[op2]
#[serde]
pub fn op_chq_url_parse_search_params(#[string] query: String) -> Vec<(String, String)> {
    let query = query.trim_start_matches('?');
    url::form_urlencoded::parse(query.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

#[op2]
#[string]
pub fn op_chq_url_stringify_search_params(#[serde] pairs: Vec<(String, String)>) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(&key, &value);
    }
    serializer.finish()
}
