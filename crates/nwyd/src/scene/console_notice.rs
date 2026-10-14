pub fn banner(
    text: &str,
    channel: i32,
    player: Option<glam::Vec3>,
    selection: bool,
) -> Option<(String, u32)> {
    if text.starts_with('^') {
        return (!selection).then(|| (text.to_owned(), 7000));
    }

    if text.starts_with("!#") {
        return None;
    }

    if text.chars().skip(1).take(3).eq(['!', '!', '!']) {
        return player.map(|_| (text.to_owned(), 4000));
    }

    if text.starts_with("!!") {
        return None;
    }

    let text = if text.starts_with("200") {
        match player {
            Some(at) => format!("{text} {channel} {},{}", at.x as i32, at.z as i32),
            None => format!("{text} {channel}"),
        }
    } else {
        text.to_owned()
    };

    Some((text, 4000))
}
