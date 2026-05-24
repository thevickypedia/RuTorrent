use base64::{engine::general_purpose, Engine as _};
use crate::{constant, settings};
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use minijinja::{context, Environment};
use serde_json::json;

fn build_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.add_template("index.html", include_str!("templates/index.html"))
        .unwrap();
    env
}

pub async fn index_page(
    metadata: web::Data<constant::MetaData>,
) -> impl Responder {
    let env = build_env();
    let tmpl = env.get_template("index.html").unwrap();
    let rendered = tmpl
        .render(context! {
            version => metadata.pkg_version,
        })
        .unwrap();
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(rendered)
}

fn base64_decode(value: &str) -> Result<String, Box<dyn std::error::Error>> {
    let decoded = general_purpose::STANDARD.decode(value)?;
    let result = String::from_utf8(decoded)?;
    Ok(result)
}

pub async fn authenticator(
    request: HttpRequest,
    config: web::Data<settings::Config>,
) -> impl Responder {
    let auth_header = match request.headers().get("Authorization") {
        Some(head) => head,
        None => {
            log::warn!("No Authorization Header received");
            return HttpResponse::Unauthorized().finish()
        },
    };
    let encoded = match auth_header.to_str() {
        Ok(header) => header,
        Err(err) => {
            log::warn!("{}", err);
            return HttpResponse::Unauthorized().finish()
        },
    };
    let decoded = match base64_decode(encoded) {
        Ok(decoded_) => decoded_,
        Err(err) => {
            log::warn!("{}", err);
            return HttpResponse::Unauthorized().finish()
        },
    };
    let auth_header = decoded.split(",").collect::<Vec<&str>>();
    if auth_header.len() != 2 {
        log::warn!("Expected two Authorization headers, received {:?}", auth_header);
        return HttpResponse::Unauthorized().finish();
    }
    let username = auth_header.first().unwrap().to_string();
    let password = auth_header.last().unwrap().to_string();
    if username == config.username && password == config.password {
        return HttpResponse::Ok().json(json!({ "apikey": config.apikey }))
    }
    log::warn!("Username and password do not match");
    HttpResponse::Unauthorized().finish()
}
