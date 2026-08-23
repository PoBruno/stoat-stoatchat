use crate::File;

auto_derived_partial!(
    /// A post in a forum channel
    pub struct ForumPost {
        /// Unique Id
        #[serde(rename = "_id")]
        pub id: String,

        /// Id of the forum channel this post belongs to
        pub channel: String,
        /// Id of the server the channel belongs to
        pub server: String,
        /// Id of the user who created this post
        pub author: String,

        /// Post title
        pub title: String,
        /// Post body, markdown
        #[serde(skip_serializing_if = "Option::is_none")]
        pub content: Option<String>,
        /// Attached files
        #[serde(skip_serializing_if = "Option::is_none")]
        pub attachments: Option<Vec<File>>,

        /// Ids of tags applied to this post
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub tags: Vec<String>,

        /// Ids of users who upvoted
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub upvoters: Vec<String>,
        /// Number of upvotes, kept in sync with `upvoters`
        #[serde(default)]
        pub score: i32,
        /// Number of comments
        #[serde(default)]
        pub comment_count: i32,
        /// Score decayed by age, fixed point at 1e9
        ///
        /// Stored as an integer rather than a float so the model can derive
        /// `Eq` (which the partial-struct macro requires) and so ordering is
        /// exact rather than subject to float comparison.
        #[serde(default)]
        pub hot_rank: i64,

        /// Ids of users who have opened this post
        ///
        /// Stored as a set rather than a counter so a refresh does not inflate
        /// the number. At ten people that list stays tiny.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub viewers: Vec<String>,

        /// Ids of users following this post
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub subscribers: Vec<String>,

        /// When the most recent comment was posted
        #[serde(skip_serializing_if = "Option::is_none")]
        pub last_comment_at: Option<String>,

        /// Whether this post is pinned to the top of the feed
        #[serde(default, skip_serializing_if = "crate::if_false")]
        pub pinned: bool,
        /// Whether this post is closed to new comments
        #[serde(default, skip_serializing_if = "crate::if_false")]
        pub locked: bool,

        /// When this post was last edited
        #[serde(skip_serializing_if = "Option::is_none")]
        pub edited: Option<String>,

        /// Id of whoever deleted this post
        #[serde(skip_serializing_if = "Option::is_none")]
        pub deleted_by: Option<String>,
    },
    "PartialForumPost"
);

auto_derived!(
    /// Fields that can be removed from a forum post
    pub enum FieldsForumPost {
        Content,
        Attachments,
    }
);

/// Hours after which a post stops decaying and drops out of the hot feed.
///
/// Lemmy uses the same cutoff; it is what lets the recompute stay bounded and
/// makes stale posts fall through to the secondary ordering on their own.
const HOT_RANK_MAX_AGE_HOURS: f64 = 168.0;

/// Scale applied when storing `hot_rank` as a fixed point integer.
const HOT_RANK_SCALE: f64 = 1_000_000_000.0;

/// Score decayed by age, ported from Lemmy's `r.hot_rank`.
///
/// `log(max(2, score + 2)) / (hours + 2)^1.8`, hard-zeroed outside 0..7 days,
/// then scaled to a fixed point integer.
pub fn hot_rank(score: i32, created_at_ms: i64, now_ms: i64) -> i64 {
    let hours = (now_ms - created_at_ms) as f64 / 3_600_000.0;

    if hours <= 0.0 || hours >= HOT_RANK_MAX_AGE_HOURS {
        return 0;
    }

    let numerator = f64::max(2.0, score as f64 + 2.0).log10();
    let rank = numerator / (hours + 2.0).powf(1.8);

    (rank * HOT_RANK_SCALE) as i64
}

#[cfg(test)]
mod tests {
    use super::hot_rank;

    const HOUR: i64 = 3_600_000;

    #[test]
    fn decays_with_age() {
        let now = 1_000 * HOUR;
        let fresh = hot_rank(10, now - HOUR, now);
        let older = hot_rank(10, now - 24 * HOUR, now);
        assert!(fresh > older, "{fresh} should beat {older}");
    }

    #[test]
    fn rises_with_score() {
        let now = 1_000 * HOUR;
        let low = hot_rank(1, now - HOUR, now);
        let high = hot_rank(100, now - HOUR, now);
        assert!(high > low);
    }

    #[test]
    fn zeroes_outside_window() {
        let now = 1_000 * HOUR;
        assert_eq!(hot_rank(50, now - 200 * HOUR, now), 0, "older than 7d");
        assert_eq!(hot_rank(50, now + HOUR, now), 0, "scheduled in future");
    }

    #[test]
    fn unvoted_post_still_ranks() {
        let now = 1_000 * HOUR;
        assert!(hot_rank(0, now - HOUR, now) > 0);
    }

    #[test]
    fn fresh_low_score_beats_old_high_score() {
        // the whole point of the decay
        let now = 1_000 * HOUR;
        let fresh = hot_rank(2, now - HOUR, now);
        let stale = hot_rank(500, now - 120 * HOUR, now);
        assert!(fresh > stale, "fresh {fresh} vs stale {stale}");
    }
}
