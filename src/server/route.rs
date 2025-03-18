use std::collections::HashMap;

use crate::{
    config::RouteConfig,
    http::{status::StatusCode, Response},
};

pub struct Route {
    pub path: String,
    pub root: Option<String>,
    pub index: Option<String>,
    pub methods: Vec<String>,
    pub directory_listing: bool,
    pub redirect: Option<Redirect>,
    pub cgi: Option<HashMap<String, String>>,
    pub client_max_body_size: Option<String>,
}

pub struct Redirect {
    pub url: String,
    pub code: u16,
}

impl Route {
    pub fn handle(&self) -> Response {
        if let Some(_) = &self.redirect {
            return self.handle_redirect();
        } else if let Some(_) = &self.cgi {
            return self.handle_cgi();
        } else {
            return self.handle_static();
        }
    }

    fn handle_redirect(&self) -> Response {
        let mut response =
            Response::new(StatusCode::from_u16(self.redirect.as_ref().unwrap().code).unwrap());
        response.set_header("Location", &self.redirect.as_ref().unwrap().url);
        response
    }

    fn handle_cgi(&self) -> Response {
        let response = Response::new(StatusCode::Ok);
        response
    }

    fn handle_static(&self) -> Response {
        let response = Response::new(StatusCode::Ok);
        response
    }
}

impl From<RouteConfig> for Route {
    fn from(config: RouteConfig) -> Self {
        Route {
            path: config.path,
            root: config.root,
            index: config.index,
            methods: config.methods,
            directory_listing: config.directory_listing,
            redirect: config.redirect.map(|r| Redirect {
                url: r.url,
                code: r.code,
            }),
            cgi: config.cgi,
            client_max_body_size: config.client_max_body_size,
        }
    }
}
