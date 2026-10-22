use std::collections::HashMap;

/// Returns nested categories as a `HashMap<String, HashMap<u16, String>>` object.
pub fn categories() -> HashMap<String, HashMap<u16, String>> {
    let mut map = HashMap::new();

    let mut audio = HashMap::new();
    audio.insert(100, "Audio".into());
    audio.insert(101, "Music".into());
    audio.insert(102, "Audio Books".into());
    audio.insert(103, "Sound Clips".into());
    audio.insert(104, "FLAC".into());
    audio.insert(199, "Other (Audio)".into());
    map.insert("Audio".into(), audio);

    let mut video = HashMap::new();
    video.insert(200, "Video".into());
    video.insert(201, "Movies".into());
    video.insert(202, "Movies DVDR".into());
    video.insert(203, "Music Videos".into());
    video.insert(204, "Movie Clips".into());
    video.insert(205, "TV Shows".into());
    video.insert(206, "Handheld".into());
    video.insert(207, "HD – Movies".into());
    video.insert(208, "HD – TV Shows".into());
    video.insert(209, "3D".into());
    video.insert(210, "CAM/TS".into());
    video.insert(211, "UHD/4k – Movies".into());
    video.insert(212, "UHD/4k – TV Shows".into());
    video.insert(299, "Other (Video)".into());
    map.insert("Video".into(), video);

    let mut apps = HashMap::new();
    apps.insert(300, "Applications".into());
    apps.insert(301, "Windows".into());
    apps.insert(302, "Mac".into());
    apps.insert(303, "UNIX".into());
    apps.insert(304, "Handheld".into());
    apps.insert(305, "iOS (iPad/iPhone)".into());
    apps.insert(306, "Android".into());
    apps.insert(399, "Other OS".into());
    map.insert("Applications".into(), apps);

    let mut games = HashMap::new();
    games.insert(400, "Games".into());
    games.insert(401, "PC".into());
    games.insert(402, "Mac".into());
    games.insert(403, "PSx".into());
    games.insert(404, "XBOX360".into());
    games.insert(405, "Wii".into());
    games.insert(406, "Handheld".into());
    games.insert(407, "iOS (iPad/iPhone)".into());
    games.insert(408, "Android".into());
    games.insert(499, "Other (Games)".into());
    map.insert("Games".into(), games);

    let mut porn = HashMap::new();
    porn.insert(500, "Porn".into());
    porn.insert(501, "Movies".into());
    porn.insert(502, "Movies DVDR".into());
    porn.insert(503, "Pictures".into());
    porn.insert(504, "Games".into());
    porn.insert(505, "HD – Movies".into());
    porn.insert(506, "Movie Clips".into());
    porn.insert(507, "UHD/4k – Movies".into());
    porn.insert(599, "Other (XXX)".into());
    map.insert("Porn".into(), porn);

    let mut other = HashMap::new();
    other.insert(600, "Other".into());
    other.insert(601, "E-books".into());
    other.insert(602, "Comics".into());
    other.insert(603, "Pictures".into());
    other.insert(604, "Covers".into());
    other.insert(605, "Physibles".into());
    other.insert(699, "Other (Other)".into());
    map.insert("Other".into(), other);

    map
}

/// Use the category id to map the base category name.
///
/// # Arguments
///
/// * `id` - Category id.
///
/// # Returns
///
/// Returns the parent category name as an `Option<String>` object.
pub fn base_map(id: &u16) -> Option<String> {
    if id < &100 {
        return None;
    };
    match *id / 100 {
        1 => Some("Audio".to_string()),
        2 => Some("Video".to_string()),
        3 => Some("Applications".to_string()),
        4 => Some("Games".to_string()),
        5 => Some("Porn".to_string()),
        6 => Some("Other".to_string()),
        _ => None,
    }
}

/// Flattens the categories without the base keys.
///
/// # Arguments
///
/// * `categories` - Takes the nested categories as an `HashMap<String, HashMap<u16, String>>` object.
///
/// # Returns
///
/// Returns the flattened category map as an `HashMap<u16, String>` object.
pub fn flatten(categories: &HashMap<String, HashMap<u16, String>>) -> HashMap<u16, String> {
    categories
        .values()
        .flat_map(|inner| inner.iter())
        .map(|(k, v)| (*k, v.clone()))
        .collect()
}
