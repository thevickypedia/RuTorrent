/// Entry point for the RuTorrent application.
///
/// This asynchronous main function triggers the `start` function from the `rutorrent` library.
#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    rutorrent::start().await
}

// Magnet links are hard-coded since the JSON API often lacks trackers
// #[tokio::main]
// async fn main1() -> Result<(), Box<dyn std::error::Error>> {
//     use rbay::{Category, search, torrent, CATEGORIES};
//
//     let movies = Category::new(201).unwrap();
//     assert_eq!(movies.name(), "Video: Movies");
//
//     let torrents = search("Barbie", Some(movies)).await?;
//     println!("Found {} torrents", torrents.len());
//
//     let first = &torrents[0];
//     println!("First torrent: {}", first.name);
//     println!("Magnet link: {}", first.magnet());
//
//     let torrent = torrent(first.id).await?;
//     println!("Description: {}", torrent.descr);
//
//     Ok(())
// }
