use std::collections::HashMap;
use actix_web::{web, HttpRequest, Responder, HttpResponse};
use actix_web::http::header::{HeaderName, HeaderValue};
use crate::{api, config, search};

/// API endpoint to get search category.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `nested` - Boolean flag to get the nested object.
///
/// #### Sample Request
/// ```shell
/// curl localhost:3000/search-categories?nested=false
/// ```
///
/// #### Sample Response
/// ```json
/// {
///   "100": "Audio",
///   "101": "Music",
///   "102": "Audio Books",
///   "103": "Sound Clips",
///   ...
/// }
/// ```
///
/// #### Status
/// * `200`: Successful.
///
/// # Returns
///
/// Returns a HashMap of categories with id and name as key-value pairs.
#[utoipa::path(
    get,
    tag = "Search",
    path = "/categories",
    params(
        ("nested" = Option<bool>, Query, description = "Boolean flag to get nested categories")
    ),
    responses(
        (status = 200, description = "Nested or flattened category map", body = api::schema::GetCategoriesResponse),
    )
)]
pub async fn category(
    request: HttpRequest,
    query: web::Query<HashMap<String, String>>,
    config: web::Data<config::env::Config>,
) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let nested = query.get("nested").map(|v| v == "true").unwrap_or(false);
    let categories = &search::category::categories();
    if nested {
        log::info!("Rendering nested categories");
        HttpResponse::Ok().json(categories)
    } else {
        log::info!("Rendering flattened categories");
        HttpResponse::Ok().json(search::category::flatten(categories))
    }
}

/// API endpoint to search for torrents.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `payload` - Reference to the `SearchQuery` object.
/// * `config` - Reference to the `Config` object.
///
/// #### Sample Request
/// ```shell
/// curl localhost:3000/search?query=ubuntu&category=0&hydrate_categories=false&use_tor=true
/// ```
///
/// #### Sample Response
/// ```json
/// [
///     {
///         "title": "Ubuntu 22.04 LTS",
///         "category": "303",
///         "size": "3.40 GB",
///         "seeders": 40,
///         "leechers": 12,
///         "magnet_link": "magnet:?xt=urn:btih:2C6B6858D61DA9543D4231A71DB4B1C9264B0685&dn=Ubuntu%2022.04%20LTS",
///         "uploader": "rjaa",
///         "date_uploaded": "1652877231"
///     },
///     {
///         "title": "Clinton D., Negus C. Ubuntu Linux Bible 11ed 2025",
///         "category": "601",
///         "size": "8.16 MB",
///         "seeders": 27,
///         "leechers": 0,
///         "magnet_link": "magnet:?xt=urn:btih:232CD67EB3FFBD7C37BF9EC3EE887417E5AE1EE6&dn=Clinton%20D.%2C%20Negus%20C.%20Ubuntu%20Linux%20Bible%2011ed%202025",
///         "uploader": "andryold1",
///         "date_uploaded": "1765526528"
///     }
/// ]
/// ```
///
/// #### Status
/// * `200`: Successful.
/// * `404`: Not found.
///
/// # Returns
///
/// Returns a JSON array of `SearchTorrentResponse` objects.
#[utoipa::path(
    get,
    tag = "Search",
    path = "/search",
    params(api::schema::SearchTorrentRequest),
    responses(
        (status = 200, description = "List of results as structured object", body = Vec<api::schema::SearchTorrentResponse>),
        (status = 404, description = "No results", body = String),
        (status = 502),
        (status = 503),
        (status = 504),
    )
)]
pub async fn torrents(
    request: HttpRequest,
    payload: web::Query<api::schema::SearchTorrentRequest>,
    config: web::Data<config::env::Config>,
    metadata: web::Data<config::constant::MetaData>,
) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    if payload.use_tor && config.tor_url.is_empty() {
        return HttpResponse::FailedDependency().json("Host does not support tor");
    }
    log::debug!("Query: {:?}", payload);
    let torrents = match search::explore::search(
        &payload.query,
        payload.category,
        payload.use_tor,
        payload.hydrate_categories,
        &config,
        &metadata,
    )
        .await
    {
        Ok(torrents) => torrents,
        Err(e) => {
            log::error!("Failed to search for {} - {}", payload.query, e);
            let status = if let Some(h) = e.downcast_ref::<api::schema::HttpError>() {
                actix_web::http::StatusCode::from_u16(h.status)
                    .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR)
            } else if let Some(r) = e.downcast_ref::<reqwest::Error>() {
                if r.is_timeout() {
                    actix_web::http::StatusCode::GATEWAY_TIMEOUT
                } else if r.is_builder() {
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
                } else {
                    actix_web::http::StatusCode::BAD_GATEWAY
                }
            } else {
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
            };
            return HttpResponse::build(status).json(e.to_string());
        }
    };
    let length = torrents.len();
    let mut response = HttpResponse::Ok().json(torrents);
    response.headers_mut().insert(
        HeaderName::from_static("results"),
        HeaderValue::from(length),
    );
    response
}
