use revolt_rocket_okapi::revolt_okapi::openapi3::OpenApi;
use rocket::Route;

mod agent;
mod play;
pub mod queue;
mod resolve;
pub mod state;
mod token;

pub fn routes() -> (Vec<Route>, OpenApi) {
    openapi_get_routes_spec![
        agent::heartbeat,
        agent::take_work,
        agent::hand_in,
        play::progress,
        resolve::resolve,
        token::agent_token,
        play::fetch_queue,
        play::enqueue,
        play::dequeue,
        play::clear_queue,
        play::play_queued,
        play::next,
        play::toggle,
        play::stop,
        play::settings
    ]
}
