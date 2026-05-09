/// Entry point for the RuTorrent application.
///
/// This asynchronous main function triggers the `start` function from the `rutorrent` library.
#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    rutorrent::start().await
}
