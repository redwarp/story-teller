use twee_v3::Story;

pub fn story_title(story: &str) -> Option<String> {
    let story = Story::try_from(story);
    if story.is_err() {
        return None;
    }
    let story = story.unwrap();
    return story.title().map(ToString::to_string);
}
