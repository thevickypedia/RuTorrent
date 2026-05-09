use crate::{api, settings};
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
    components(schemas(settings::PutItem)),
    security(
        ("apikey" = [])
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "apikey",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("apikey"))),
        );
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
