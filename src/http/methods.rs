use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    GET,
    POST,
    DELETE,
    HEAD,
    PUT,
    PATCH,
    OPTIONS,
    UNKNOWN(String),
}

impl Method {
    pub fn as_str(&self) -> &str {
        match self {
            Method::GET => "GET",
            Method::POST => "POST",
            Method::DELETE => "DELETE",
            Method::HEAD => "HEAD",
            Method::PUT => "PUT",
            Method::PATCH => "PATCH",
            Method::OPTIONS => "OPTIONS",
            Method::UNKNOWN(s) => s,
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "GET" => Method::GET,
            "POST" => Method::POST,
            "DELETE" => Method::DELETE,
            "HEAD" => Method::HEAD,
            "PUT" => Method::PUT,
            "PATCH" => Method::PATCH,
            "OPTIONS" => Method::OPTIONS,
            _ => Method::UNKNOWN(s.to_string()),
        }
    }
}

impl Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}