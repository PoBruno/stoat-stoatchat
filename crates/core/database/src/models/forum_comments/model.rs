use crate::File;

auto_derived_partial!(
    /// A comment on a forum post
    ///
    /// The tree lives in `ancestors`: the ordered list of comment ids from the
    /// root down to this comment's parent. A whole thread is one query plus an
    /// in-memory assembly, which is the right trade at this scale (a dozen
    /// posts a year, ten people).
    pub struct ForumComment {
        /// Unique Id
        #[serde(rename = "_id")]
        pub id: String,

        /// Id of the post this comment belongs to
        pub post: String,
        /// Id of the forum channel, denormalised so a channel delete can sweep
        pub channel: String,
        /// Id of the user who wrote this comment
        pub author: String,

        /// Comment body, markdown
        pub content: String,
        /// Attached files
        #[serde(skip_serializing_if = "Option::is_none")]
        pub attachments: Option<Vec<File>>,

        /// Id of the comment being replied to, absent for a top level comment
        #[serde(skip_serializing_if = "Option::is_none")]
        pub parent: Option<String>,
        /// Ids from the root down to `parent`, empty for a top level comment
        ///
        /// Depth is `ancestors.len()`; there is no cap.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub ancestors: Vec<String>,

        /// Ids of users who upvoted
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub upvoters: Vec<String>,
        /// Number of upvotes, kept in sync with `upvoters`
        #[serde(default)]
        pub score: i32,

        /// When this comment was last edited
        #[serde(skip_serializing_if = "Option::is_none")]
        pub edited: Option<String>,

        /// Id of whoever deleted this comment
        ///
        /// Deleting is a tombstone rather than a removal: dropping a comment
        /// with replies would orphan the subtree.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub deleted_by: Option<String>,
    },
    "PartialForumComment"
);

auto_derived!(
    /// Fields that can be removed from a forum comment
    pub enum FieldsForumComment {
        Attachments,
    }
);
