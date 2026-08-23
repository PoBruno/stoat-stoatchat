use revolt_rocket_okapi::revolt_okapi::openapi3::OpenApi;
use rocket::Route;

mod channel_ack;
mod channel_delete;
mod channel_edit;
mod forum_comment_create;
mod forum_comment_edit;
mod forum_post_create;
mod forum_post_edit;
mod forum_post_fetch;
mod forum_post_query;
mod forum_post_subscribe;
mod forum_post_vote;
mod forum_settings;
mod sound_play;
mod channel_fetch;
mod group_add_member;
mod group_create;
mod group_remove_member;
mod invite_create;
mod members_fetch;
mod message_bulk_delete;
mod message_clear_reactions;
mod message_delete;
mod message_edit;
mod message_fetch;
mod message_pin;
mod message_query;
mod message_react;
mod message_search;
mod message_send;
mod message_unpin;
mod message_unreact;
mod permissions_set;
mod permissions_set_default;
mod voice_join;
mod voice_stop_ring;
mod webhook_create;
mod webhook_fetch_all;

pub fn routes() -> (Vec<Route>, OpenApi) {
    openapi_get_routes_spec![
        channel_ack::ack,
        channel_fetch::fetch,
        members_fetch::fetch_members,
        channel_delete::delete,
        channel_edit::edit,
        invite_create::create_invite,
        forum_post_create::create_forum_post,
        forum_post_query::query_forum_posts,
        forum_post_fetch::fetch_forum_post,
        forum_post_edit::edit_forum_post,
        forum_post_edit::delete_forum_post,
        forum_post_vote::upvote_forum_post,
        forum_post_vote::remove_forum_post_upvote,
        forum_comment_create::create_forum_comment,
        forum_comment_create::query_forum_comments,
        forum_comment_edit::edit_forum_comment,
        forum_comment_edit::delete_forum_comment,
        forum_comment_edit::upvote_forum_comment,
        forum_comment_edit::remove_forum_comment_upvote,
        forum_settings::edit_forum_settings,
        sound_play::play_sound,
        forum_post_subscribe::subscribe_forum_post,
        forum_post_subscribe::unsubscribe_forum_post,
        message_send::message_send,
        message_query::query,
        message_search::search,
        message_pin::message_pin,
        message_fetch::fetch,
        message_edit::edit,
        message_bulk_delete::bulk_delete_messages,
        message_delete::delete,
        message_unpin::message_unpin,
        group_create::create_group,
        group_add_member::add_member,
        group_remove_member::remove_member,
        voice_join::call,
        voice_stop_ring::stop_ring,
        permissions_set::set_role_permissions,
        permissions_set_default::set_default_channel_permissions,
        message_react::react_message,
        message_unreact::unreact_message,
        message_clear_reactions::clear_reactions,
        webhook_create::create_webhook,
        webhook_fetch_all::fetch_webhooks,
    ]
}
