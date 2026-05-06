use crate::api;
use actix_web::HttpResponse;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::Modify;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
#[derive(OpenApi)]
#[openapi(
    paths(
        api::status,
        api::version,
        api::get_torrents,
        api::put_torrent,
        api::delete_torrent
    ),
    security(
        ("apikey" = [])
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let mut components = utoipa::openapi::Components::new();
        components.add_security_scheme(
            "apikey",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("apikey"))),
        );
        openapi.components = Some(components);
    }
}

pub fn service() -> SwaggerUi {
    let openapi = ApiDoc::openapi();
    SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", openapi)
}

pub async fn redirector() -> HttpResponse {
    HttpResponse::Found()
        .append_header(("Location", "/swagger-ui/"))
        .finish()
}
