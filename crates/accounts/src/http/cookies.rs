//! Cookie construction and parsing.

use axum::http::{HeaderMap, header};
use cookie::time::Duration;
use cookie::{Cookie, SameSite};

use crate::API_PREFIX;

/// The session cookie: opaque random token, HttpOnly, SameSite=Lax, Secure outside development.
pub const SESSION_COOKIE: &str = "hf_session";
/// Anti-forgery state of an in-flight GitHub sign-in.
pub const GITHUB_STATE_COOKIE: &str = "hf_gh_state";

fn base(
    name: &'static str,
    value: String,
    secure: bool,
    path: String,
) -> cookie::CookieBuilder<'static> {
    Cookie::build((name, value))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure)
        .path(path)
}

pub fn session_cookie(token: &str, lifetime_seconds: i64, secure: bool) -> String {
    base(SESSION_COOKIE, token.to_owned(), secure, "/".to_owned())
        .max_age(Duration::seconds(lifetime_seconds.max(0)))
        .build()
        .to_string()
}

pub fn clear_session_cookie(secure: bool) -> String {
    base(SESSION_COOKIE, String::new(), secure, "/".to_owned())
        .max_age(Duration::ZERO)
        .build()
        .to_string()
}

fn github_path() -> String {
    format!("{API_PREFIX}/auth/github")
}

/// Valid for ten minutes, sent only to the GitHub flow endpoints.
pub fn github_state_cookie(value: &str, secure: bool) -> String {
    base(GITHUB_STATE_COOKIE, value.to_owned(), secure, github_path())
        .max_age(Duration::minutes(10))
        .build()
        .to_string()
}

pub fn clear_github_state_cookie(secure: bool) -> String {
    base(GITHUB_STATE_COOKIE, String::new(), secure, github_path())
        .max_age(Duration::ZERO)
        .build()
        .to_string()
}

/// Value of cookie `name` in the request's `Cookie` headers.
pub fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    for value in headers.get_all(header::COOKIE) {
        let Ok(text) = value.to_str() else { continue };
        for cookie in Cookie::split_parse(text.to_owned()).flatten() {
            if cookie.name() == name {
                return Some(cookie.value().to_owned());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn session_cookie_attributes() {
        let prod = session_cookie("tok", 3600, true);
        assert!(prod.starts_with("hf_session=tok"));
        for attr in [
            "HttpOnly",
            "SameSite=Lax",
            "Secure",
            "Path=/",
            "Max-Age=3600",
        ] {
            assert!(prod.contains(attr), "{prod} lacks {attr}");
        }
        let dev = session_cookie("tok", 3600, false);
        assert!(!dev.contains("Secure"));
        assert!(dev.contains("HttpOnly") && dev.contains("SameSite=Lax"));
    }

    #[test]
    fn clearing_cookies_expire_immediately() {
        assert!(clear_session_cookie(true).contains("Max-Age=0"));
        let gh = clear_github_state_cookie(false);
        assert!(gh.contains("Max-Age=0") && gh.contains("Path=/api/v1/auth/github"));
    }

    #[test]
    fn reads_the_named_cookie_among_others() {
        let mut headers = HeaderMap::new();
        headers.append(
            header::COOKIE,
            HeaderValue::from_static("a=1; hf_session=abc; b=2"),
        );
        headers.append(header::COOKIE, HeaderValue::from_static("c=3"));
        assert_eq!(read_cookie(&headers, "hf_session").as_deref(), Some("abc"));
        assert_eq!(read_cookie(&headers, "c").as_deref(), Some("3"));
        assert_eq!(read_cookie(&headers, "missing"), None);
    }
}
