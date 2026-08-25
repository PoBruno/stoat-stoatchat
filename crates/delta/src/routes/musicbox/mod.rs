use revolt_rocket_okapi::revolt_okapi::openapi3::OpenApi;
use rocket::Route;

mod agent;
mod play;
mod resolve;
pub mod state;
mod token;

pub fn routes() -> (Vec<Route>, OpenApi) {
    openapi_get_routes_spec![
        agent::heartbeat,
        agent::take_work,
        agent::hand_in,
        resolve::resolve,
        token::agent_token,
        play::play,
        play::stop
    ]
}
