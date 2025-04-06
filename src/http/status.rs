use std::fmt::Display;

#[derive(Debug, Clone, Copy)]
pub enum StatusCode {
    Ok = 200,
    Created = 201,
    NoContent = 204,
    MultipleChoices = 300,
    MovedPermenantly = 301,
    Found = 302,
    SeeOther = 303,
    NotModified = 304,
    UseProxy = 305,
    Unused = 306,
    TemporaryRedirect = 307,
    PermenantRedirect = 308,
    BadRequest = 400,
    Unauthorized = 401,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    PayloadTooLarge = 413,
    InternalServerError = 500,
    NotImplemented = 501,
}

impl StatusCode {
    pub fn from_u16(status_code: u16) -> Option<Self> {
        match status_code {
            200 => Some(StatusCode::Ok),
            201 => Some(StatusCode::Created),
            204 => Some(StatusCode::NoContent),
            300 => Some(StatusCode::MultipleChoices),
            301 => Some(StatusCode::MovedPermenantly),
            302 => Some(StatusCode::Found),
            303 => Some(StatusCode::SeeOther),
            304 => Some(StatusCode::NotModified),
            305 => Some(StatusCode::UseProxy),
            306 => Some(StatusCode::Unused),
            307 => Some(StatusCode::TemporaryRedirect),
            308 => Some(StatusCode::PermenantRedirect),
            400 => Some(StatusCode::BadRequest),
            401 => Some(StatusCode::Unauthorized),
            403 => Some(StatusCode::Forbidden),
            404 => Some(StatusCode::NotFound),
            405 => Some(StatusCode::MethodNotAllowed),
            413 => Some(StatusCode::PayloadTooLarge),
            500 => Some(StatusCode::InternalServerError),
            501 => Some(StatusCode::NotImplemented),
            _ => None,
        }
    }

    pub fn as_u16(&self) -> u16 {
        *self as u16
    }

    pub fn to_text(&self) -> String {
        match self {
            StatusCode::Ok => "OK".to_string(),
            StatusCode::Created => "Created".to_string(),
            StatusCode::NoContent => "No Content".to_string(),
            StatusCode::MultipleChoices => "Multiple Choices".to_string(),
            StatusCode::MovedPermenantly => "Moved Permanently".to_string(),
            StatusCode::Found => "Found".to_string(),
            StatusCode::SeeOther => "See Other".to_string(),
            StatusCode::NotModified => "Not Modified".to_string(),
            StatusCode::UseProxy => "Use Proxy".to_string(),
            StatusCode::Unused => "Unused".to_string(),
            StatusCode::TemporaryRedirect => "Temporary Redirect".to_string(),
            StatusCode::PermenantRedirect => "Permenant Redirect".to_string(),
            StatusCode::BadRequest => "Bad Request".to_string(),
            StatusCode::Unauthorized => "Unauthorized".to_string(),
            StatusCode::Forbidden => "Forbidden".to_string(),
            StatusCode::NotFound => "Not Found".to_string(),
            StatusCode::MethodNotAllowed => "Method Not Allowed".to_string(),
            StatusCode::PayloadTooLarge => "Payload Too Large".to_string(),
            StatusCode::InternalServerError => "Internal Server Error".to_string(),
            StatusCode::NotImplemented => "Not Implemented".to_string(),
        }
    }
}

impl Display for StatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_u16())
    }
}