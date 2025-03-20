#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    GET,
    POST,
    DELETE,
    UNKNOWN(String),
}

impl Method {
    pub fn as_str(&self) -> &str {
        match self {
            Method::GET => "GET",
            Method::POST => "POST",
            Method::DELETE => "DELETE",
            Method::UNKNOWN(s) => s,
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "GET" => Method::GET,
            "POST" => Method::POST,
            "DELETE" => Method::DELETE,
            _ => Method::UNKNOWN(s.to_string()),
        }
    }
}