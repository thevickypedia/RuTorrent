use crate::{api};
use actix_web::HttpResponse;
use utoipa::Modify;
use utoipa::OpenApi;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa_swagger_ui::SwaggerUi;
#[derive(OpenApi)]
#[openapi(
    paths(
        api::search::torrents,
        api::search::category,

        api::torrent::get,
        api::torrent::put,
        api::torrent::pause,
        api::torrent::delete,
        api::torrent::retry,

        api::vpn::get,
        api::vpn::connect,
        api::vpn::disconnect,
        api::vpn::feature,
    ),
    components(schemas(api::schema::PutTorrentRequest, api::schema::PutTorrentResponse, api::schema::RetryTorrentRequest, api::schema::GetTorrentResponse, api::schema::SearchTorrentResponse, api::schema::MullvadResponse)),
    security(
        ("apikey" = [])
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    /// Adds an API key security scheme to the OpenAPI components.
    ///
    /// This inserts an `apikey` header-based security scheme at the top level
    /// so it is available in Swagger UI and other OpenAPI consumers.
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "apikey",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("apikey"))),
        );
    }
}

/// Service handler that returns the constructed swagger UI endpoint with openapi docs.
///
/// # Returns
///
/// Returns the `SwaggerUi` object.
pub fn service() -> SwaggerUi {
    let openapi = ApiDoc::openapi();
    SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", openapi)
}

/// Function to redirect the page to `swagger-ui` endpoint.
///
/// # Returns
///
/// Returns a `HttpResponse` with a `Location` header.
pub async fn redirector() -> HttpResponse {
    HttpResponse::Found()
        .append_header(("Location", "/swagger-ui/"))
        .finish()
}
