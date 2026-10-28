use crate::{api, config, vpn};
use actix_web::{HttpRequest, HttpResponse, Responder, web};
use std::collections::HashMap;

/// API endpoint to get the current vpn relay using mullvad cli.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `config` - Reference to the `Config` object.
///
/// #### Sample Request
/// ```shell
/// curl localhost:3000/vpn?features=false
/// ```
///
/// #### Sample Response
/// ```json
/// {
///   "state": "connected",
///   "details": {
///     "location": {
///       "city": "Madrid",
///       "country": "Spain",
///       "ipv4": null,
///       "hostname": "es-mad-wg-203"
///     },
///     "locked_down": null,
///     "feature_indicators": [
///       "QuantumResistance",
///       "LanSharing"
///     ]
///   }
/// }
/// ```
///
/// #### Status
/// * `200`: Response from mullvad.
/// * `502`: Mullvad CLI error.
///
/// # Returns
///
/// Returns `Vec<api::schema::Features>` if `?features=true`, otherwise `api::schema::MullvadResponse` object.
#[utoipa::path(
    get,
    tag = "VPN",
    path = "/vpn",
    params(
        ("features" = Option<bool>, Query, description = "Boolean flag to get VPN features.")
    ),
    responses(
        (status = 200, description = "VPNResponse object or a vector of supported features", body = api::schema::GetVpnResponse),
        (status = 502, description = "mullvad CLI error", body = String),
        (status = 503, description = "mullvad CLI missing or relay list unavailable", body = String),
    )
)]
pub async fn get(
    request: HttpRequest,
    query: web::Query<HashMap<String, String>>,
    config: web::Data<config::env::Config>,
) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let features = query.get("features").map(|v| v == "true").unwrap_or(false);
    if features {
        log::info!("Query features");
        let features: Vec<api::schema::Features> = vec![
            api::schema::Features::LanSharing,
            api::schema::Features::QuantumResistance,
            api::schema::Features::Daita,
            api::schema::Features::Multihop,
        ];
        return HttpResponse::Ok().json(features);
    }
    api::squire::vpn_response(vpn::connect::get_current().await)
}

/// API endpoint to connect to a vpn relay using mullvad cli.
///
/// - `hostname` input takes the highest priority.
///
/// - Supports both city and country codes for whitelist and blacklist.
///
/// - Check [https://api.mullvad.net/www/relays/all/](https://api.mullvad.net/www/relays/all/) to get the list of all relays.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `payload` - Reference to the `ConnectVPN` object.
/// * `config` - Reference to the `Config` object.
///
/// #### Sample Request
/// ```shell
/// curl -X POST localhost:3000/vpn \
///   -H "Content-Type: application/json" \
///   -d '{
///     "blacklist":"us,il,cl,cz",
///     "hostname":"",
///     "whitelist":"ch,se,ie,it,tyo"
///   }'
/// ```
///
/// #### Sample Response
/// ```json
/// {
///   "state": "connected",
///   "details": {
///     "location": {
///       "city": "Madrid",
///       "country": "Spain",
///       "ipv4": null,
///       "hostname": "es-mad-wg-203"
///     },
///     "locked_down": null,
///     "feature_indicators": [
///       "LanSharing"
///     ]
///   }
/// }
/// ```
///
/// #### Status
/// * `200`: Response from mullvad.
/// * `400`: Invalid user input.
/// * `502`: Mullvad CLI error.
/// * `503`: Mullvad CLI missing relay.
/// * `504`: VPN did not reach requested state.
///
/// # Returns
///
/// Returns the `api::schema::MullvadResponse` object.
#[utoipa::path(
    post,
    tag = "VPN",
    path = "/vpn",
    request_body = Option<api::schema::ConnectVPNRequest>,
    responses(
        (status = 200, description = "Response message from mullvad cli", body = api::schema::MullvadResponse),
        (status = 400, description = "Invalid hostname, or unknown/conflicting whitelist/blacklist codes", body = String),
        (status = 502, description = "mullvad CLI error", body = String),
        (status = 503, description = "mullvad CLI missing or relay list unavailable", body = String),
        (status = 504, description = "VPN did not reach the requested state", body = String),
    )
)]
pub async fn connect(
    request: HttpRequest,
    body: web::Json<api::schema::ConnectVPNRequest>,
    config: web::Data<config::env::Config>,
) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    if !body.hostname.trim().is_empty() {
        return api::squire::vpn_response(
            vpn::connect::connect_by_hostname(body.hostname.trim()).await,
        );
    }
    api::squire::vpn_response(
        vpn::connect::shuffler(
            api::squire::split_codes(&body.whitelist),
            api::squire::split_codes(&body.blacklist),
            &config,
        )
        .await,
    )
}

/// API endpoint to disconnect mullvad vpn.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `config` - Reference to the `Config` object.
///
/// #### Sample Request
/// ```shell
/// curl -X DELETE localhost:3000/vpn?lockdown=false
/// ```
///
/// #### Sample Response
/// ```json
/// {
///   "state": "disconnected",
///   "details": {
///     "location": null,
///     "locked_down": true,
///   }
/// }
/// ```
///
/// #### Status
/// * `200`: Response from mullvad.
/// * `502`: Mullvad CLI error.
/// * `503`: Mullvad CLI missing relay.
/// * `504`: VPN did not reach requested state.
///
/// # Returns
///
/// Returns the `api::schema::MullvadResponse` object.
#[utoipa::path(
    delete,
    tag = "VPN",
    path = "/vpn",
    params(
        ("lockdown" = Option<bool>, Query, description = "Boolean flag to set lockdown mode.")
    ),
    responses(
        (status = 200, description = "Disconnected", body = api::schema::MullvadResponse),
        (status = 502, description = "mullvad CLI error", body = String),
        (status = 503, description = "mullvad CLI missing", body = String),
        (status = 504, description = "VPN did not reach the requested state", body = String),
    )
)]
pub async fn disconnect(
    request: HttpRequest,
    query: web::Query<HashMap<String, String>>,
    config: web::Data<config::env::Config>,
) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let lockdown = query.get("lockdown").map(|v| v == "true").unwrap_or(false);
    log::warn!(
        "Received disconnect request with lockdown state as {}",
        lockdown
    );
    api::squire::vpn_response(vpn::connect::disconnect(lockdown).await)
}

/// API endpoint to disconnect mullvad vpn.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `config` - Reference to the `Config` object.
///
/// #### Sample Request
/// ```shell
/// curl -X PATCH localhost:3000/vpn?feature=LanSharing&switch=true
/// ```
///
/// #### Sample Response
/// ```json
/// {
///   "state": "connected",
///   "details": {
///     "location": {
///       "city": "Gothenburg",
///       "country": "Sweden",
///       "ipv4": "185.213.154.207",
///       "hostname": "se-got-wg-101"
///     },
///     "locked_down": null,
///     "feature_indicators": [
///       "Multihop",
///       "LanSharing"
///     ]
///   }
/// }
/// ```
///
/// #### Status
/// * `200`: Response from mullvad.
/// * `502`: Mullvad CLI error.
/// * `503`: Mullvad CLI missing relay.
/// * `504`: VPN did not reach requested state.
///
/// # Returns
///
/// Returns the `api::schema::MullvadResponse` object.
#[utoipa::path(
    patch,
    tag = "VPN",
    path = "/vpn",
    params(api::schema::ToggleFeatureRequest),
    responses(
        (status = 200, description = "Response message from mullvad cli", body = api::schema::MullvadResponse),
        (status = 502, description = "mullvad CLI error", body = String),
        (status = 503, description = "mullvad CLI missing", body = String)
    )
)]
pub async fn feature(
    request: HttpRequest,
    query: web::Query<api::schema::ToggleFeatureRequest>,
    config: web::Data<config::env::Config>,
) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let query = query.into_inner();
    log::info!(
        "Received request to toggle {:?} feature to {}",
        query.feature,
        query.switch
    );
    api::squire::vpn_response(vpn::connect::toggle_feature(query.feature, query.switch).await)
}
