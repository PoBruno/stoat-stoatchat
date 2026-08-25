use revolt_rocket_okapi::revolt_okapi::openapi3::OpenApi;
use rocket::Route;

mod agent;
mod resolve;
pub mod state;

pub fn routes() -> (Vec<Route>, OpenApi) {
    openapi_get_routes_spec![
        agent::heartbeat,
        agent::take_work,
        agent::hand_in,
        resolve::resolve
    ]
}
