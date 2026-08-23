use revolt_rocket_okapi::revolt_okapi::openapi3::OpenApi;
use rocket::Route;

mod emoji_create;
mod emoji_delete;
mod emoji_edit;
mod emoji_fetch;
mod sound_create;
mod sound_edit;

pub fn routes() -> (Vec<Route>, OpenApi) {
    openapi_get_routes_spec![
        emoji_create::create_emoji,
        emoji_delete::delete_emoji,
        emoji_edit::edit_emoji,
        emoji_fetch::fetch_emoji,
        sound_create::create_sound,
        sound_edit::fetch_sound,
        sound_edit::edit_sound,
        sound_edit::delete_sound
    ]
}
