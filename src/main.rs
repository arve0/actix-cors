use actix_web::client::{Client, SendRequestError};
use actix_web::http::{uri::Uri, Method};
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, ResponseError};
use futures::{future, Future};
use std::fmt;

const USAGE: &str = "Usage: GET /URL\n";

fn main() -> std::io::Result<()> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let server = HttpServer::new(|| {
        App::new()
            .data(Client::new())
            .service(web::resource("/").to(|| USAGE))
            .default_service(web::route().to_async(proxy))
    })
    .bind(["0.0.0.0:", &port].concat())?;

    println!("Listening on port {}", &port);

    server.run()
}

fn proxy(
    req: HttpRequest,
    client: web::Data<Client>,
) -> impl Future<Item = HttpResponse, Error = ProxyError> {
    is_get_method(req)
        .and_then(parse_uri)
        .and_then(|uri| proxy_request(uri, client))
}

/**
 * - catch all `default_service` does not support `web::get` method guard
 * - fn cannot branch into two different futures, https://gist.github.com/arve0/09d899a7ad718ca5623f56c5c03856ca
 * -> chain this fn instead
 */
fn is_get_method(req: HttpRequest) -> impl Future<Item = HttpRequest, Error = ProxyError> {
    if req.method() == Method::GET {
        future::ok(req)
    } else {
        future::failed(ProxyError::MethodNotSupported)
    }
}

fn parse_uri(req: HttpRequest) -> impl Future<Item = Uri, Error = ProxyError> {
    if req.path().is_empty() {
        return future::failed(ProxyError::UnableToParseUri);
    } else if let Ok(parsed) = req.path()[1..].parse::<Uri>() {
        if parsed.host() != None && is_valid_scheme(parsed.scheme_str()) {
            return future::ok(parsed);
        }
    }
    future::failed(ProxyError::UnableToParseUri)
}

fn is_valid_scheme(scheme: Option<&str>) -> bool {
    if let Some(scheme) = scheme {
        scheme == "https" || scheme == "http"
    } else {
        false
    }
}

fn proxy_request(
    uri: Uri,
    client: web::Data<Client>,
) -> impl Future<Item = HttpResponse, Error = ProxyError> {
    client
        .get(uri)
        .no_decompress()
        .send()
        .map_err(|err| match err {
            SendRequestError::Url(error) => ProxyError::RequestError(error.to_string()),
            SendRequestError::Connect(error) => ProxyError::RequestError(error.to_string()),
            _ => ProxyError::InternalServerError,
        })
        .and_then(|response| {
            let mut result = HttpResponse::build(response.status());
            let headers = response.headers().iter().filter(|(h, _)| {
                *h != "connection"
                    && *h != "access-control-allow-origin"
                    && *h != "content-length"
            });
            for (header_name, header_value) in headers {
                result.header(header_name.clone(), header_value.clone());
            }
            result.header("access-control-allow-origin", "*");
            Ok(result.streaming(response))
        })
}

#[derive(Debug)]
enum ProxyError {
    MethodNotSupported,
    UnableToParseUri,
    RequestError(String),
    InternalServerError,
}

impl fmt::Display for ProxyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ProxyError::*;

        match self {
            UnableToParseUri => write!(f, "Unable to parse URL\n{}", USAGE),
            RequestError(reason) => write!(f, "{}\n{}", reason, USAGE),
            _ => write!(f, "{}", USAGE),
        }
    }
}

impl ResponseError for ProxyError {
    fn error_response(&self) -> HttpResponse {
        use ProxyError::*;
        match self {
            MethodNotSupported => HttpResponse::MethodNotAllowed().finish(),
            UnableToParseUri => HttpResponse::BadRequest().finish(),
            RequestError(_) => HttpResponse::BadRequest().finish(),
            InternalServerError => HttpResponse::InternalServerError().finish(),
        }
    }
}
