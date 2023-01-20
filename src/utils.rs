use twee_v3::Story;

pub fn verify_story(story: &str) -> bool {
    let story = Story::try_from(story);
    if story.is_err() {
        return false;
    }
    let story = story.unwrap();
    return story.title().is_some();
}
