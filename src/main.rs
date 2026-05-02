#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    rutorrent::start().await
}
