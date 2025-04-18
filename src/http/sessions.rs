use std::{char::CharTryFromError, collections::HashMap};

use chrono::{DateTime, Duration, Utc};

use super::{cookies::Cookie, Request};


pub struct Session {
    pub id: String,
    pub data: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>
}

pub struct SessionStore {
    sessions: HashMap<String, Session>,
    pub session_timeout: Duration,
    pub cookie_path: String,
    pub cookie_secure: bool,
    pub cookie_http_only: bool,
}

impl SessionStore {
    pub fn new(timeout_mins: i64) -> Self {
        SessionStore {
            sessions: HashMap::new(),
            session_timeout: Duration::minutes(timeout_mins),
            cookie_path: "/".to_string(),
            cookie_secure: false,
            cookie_http_only: true,
        }
    }

    pub fn generate_id() -> String {
        // Cryptographically secure random ID
        uuid::Uuid::new_v4().to_string()
    }

    pub fn get_session(&mut self, id: &str) -> Option<&mut Session> {
        // Return session if exists and not expired
        if let Some(session) = self.sessions.get_mut(id) {
            session.last_accessed = Utc::now();
            Some(session)
        } else {
            None
        }
    }

    pub fn create_session(&mut self) -> &mut Session {
        let id = Self::generate_id();

        let session = Session {
            id: id.clone(),
            data: HashMap::new(),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };

        self.sessions.insert(id.clone(), session);
        self.sessions.get_mut(&id).unwrap()
    }

    pub fn create_session_cookie(&self, session_id: &str) -> Cookie {
        // Generate cookie with session ID
        Cookie::new(
            "session_id",
            session_id,
            Some(Utc::now() + self.session_timeout),
            Some(&self.cookie_path),
            None,
            Some(self.cookie_secure),
            Some(self.cookie_http_only),
        )
    }

    pub fn cleanup_expired(&mut self) {
        // Remove sessions past expiration
        self.sessions.retain(|_, session| {
            session.last_accessed >= Utc::now() - self.session_timeout
        });
    }

    pub fn session_from_request(&mut self, request: &Request) -> &mut Session {
    }
}