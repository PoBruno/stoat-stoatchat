use std::time::Duration;

use log::info;
use revolt_database::{hot_rank, Database};
use revolt_result::Result;
use tokio::time::sleep;

/// How often the hot feed is recomputed.
///
/// `hot_rank` is stored, not computed at read time, because the Hot feed sorts
/// on it with an index. Stored means it goes stale: a post written yesterday
/// keeps yesterday's rank and floats above fresher ones until something writes
/// to it. This task is what makes the decay actually decay.
const INTERVAL: Duration = Duration::from_secs(15 * 60);

pub async fn task(db: Database, _: revolt_database::AMQP) -> Result<()> {
    loop {
        // Only posts inside the decay window matter: `hot_rank` hard-zeroes
        // past seven days, so anything older is already at its final value.
        let posts = db.fetch_forum_posts_within_hot_window().await?;

        let now = revolt_database::iso8601_timestamp::Timestamp::now_utc()
            .duration_since(revolt_database::iso8601_timestamp::Timestamp::UNIX_EPOCH)
            .whole_milliseconds() as i64;

        let mut updated = 0;
        for post in posts {
            let created = revolt_database::ulid::Ulid::from_string(&post.id)
                .map(|u| u.timestamp_ms() as i64)
                .unwrap_or(0);

            let rank = hot_rank(post.score, created, now);
            if rank != post.hot_rank {
                db.set_forum_post_hot_rank(&post.id, rank).await?;
                updated += 1;
            }
        }

        if updated > 0 {
            info!("Recomputed hot rank for {updated} forum post(s)");
        }

        sleep(INTERVAL).await;
    }
}
