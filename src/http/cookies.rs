use std::fmt;

use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub expires: Option<String>,
    pub path: Option<String>,
    pub domain: Option<String>,
    pub secure: Option<bool>,
    pub http_only: Option<bool>,
}

impl Cookie {
    pub fn new(name: &str, value: &str) -> Self {
        Cookie {
            name: name.to_string(),
            value: value.to_string(),
            expires: None,
            path: None,
            domain: None,
            secure: None,
            http_only: None,
        }
    }

    pub fn set_expires(&mut self, expires: DateTime<Utc>) {
        self.expires = Some(expires.to_rfc2822());
    }

    pub fn set_path(&mut self, path: &str) {
        self.path = Some(path.to_string());
    }

    pub fn set_domain(&mut self, domain: &str) {
        self.domain = Some(domain.to_string());
    }

    pub fn set_secure(&mut self, secure: bool) {
        self.secure = Some(secure);
    }

    pub fn set_http_only(&mut self, http_only: bool) {
        self.http_only = Some(http_only);
    }

    pub fn to_string(&self) -> String {
        let mut cookie_str = format!("{}={}", self.name, self.value);

        if let Some(expires) = &self.expires {
            cookie_str.push_str(&format!("; expires={}", expires));
        }

        if let Some(path) = &self.path {
            cookie_str.push_str(&format!("; path={}", path));
        }

        if let Some(domain) = &self.domain {
            cookie_str.push_str(&format!("; domain={}", domain));
        }

        if let Some(secure) = self.secure {
            if secure {
                cookie_str.push_str("; secure");
            }
        }

        if let Some(http_only) = self.http_only {
            if http_only {
                cookie_str.push_str("; HttpOnly");
            }
        }

        cookie_str
    }
}

impl fmt::Display for Cookie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl From<(&str, &str)> for Cookie {
    fn from(value: (&str, &str)) -> Self {
        Cookie::new(value.0, value.1)
    }
}