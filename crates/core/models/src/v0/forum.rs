use super::File;

auto_derived_partial!(
    /// A post in a forum channel
    ///
    /// Unlike a message, a post carries a title and its own vote/comment
    /// counters so feeds can be ordered without touching the message stream.
    pub struct ForumPost {
        /// Unique Id
        #[cfg_attr(feature = "serde", serde(rename = "_id"))]
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
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        pub content: Option<String>,
        /// Attached files
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        pub attachments: Option<Vec<File>>,

        /// Ids of tags applied to this post, from the channel's available tags
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Vec::is_empty")
        )]
        pub tags: Vec<String>,

        /// Ids of users who upvoted
        ///
        /// Bounded by the size of the instance, which is why it is embedded
        /// rather than living in its own collection.
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Vec::is_empty")
        )]
        pub upvoters: Vec<String>,
        /// Number of upvotes; kept in sync with `upvoters` so feeds can sort
        #[cfg_attr(feature = "serde", serde(default))]
        pub score: i32,
        /// Number of comments
        #[cfg_attr(feature = "serde", serde(default))]
        pub comment_count: i32,

        /// How many distinct people opened this post
        ///
        /// A count, not the list: who read what is nobody else's business.
        #[cfg_attr(feature = "serde", serde(default))]
        pub views: i32,

        /// Ids of users following this post
        #[cfg_attr(feature = "serde", serde(default))]
        pub subscribers: Vec<String>,

        /// When the most recent comment was posted
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        pub last_comment_at: Option<String>,

        /// Whether this post is pinned to the top of the feed
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "crate::if_false")
        )]
        pub pinned: bool,
        /// Whether this post is closed to new comments
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "crate::if_false")
        )]
        pub locked: bool,

        /// When this post was last edited
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        pub edited: Option<String>,

        /// Id of whoever deleted this post
        ///
        /// Compare against `author` to tell "author deleted" from "moderator
        /// removed". A deleted post is kept as a tombstone while it has
        /// comments.
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
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

    /// Create a forum post
    #[cfg_attr(feature = "validator", derive(validator::Validate))]
    pub struct DataCreateForumPost {
        /// Post title
        #[cfg_attr(feature = "validator", validate(length(min = 1, max = 120)))]
        pub title: String,

        /// Post body
        #[cfg_attr(feature = "validator", validate(length(min = 0, max = 8000)))]
        pub content: Option<String>,

        /// Ids of attachments
        pub attachments: Option<Vec<String>>,

        /// Tag ids, must exist on the channel
        #[cfg_attr(feature = "serde", serde(default))]
        pub tags: Vec<String>,
    }

    /// Edit a forum post
    #[cfg_attr(feature = "validator", derive(validator::Validate))]
    pub struct DataEditForumPost {
        /// Post title
        #[cfg_attr(feature = "validator", validate(length(min = 1, max = 120)))]
        pub title: Option<String>,

        /// Post body
        #[cfg_attr(feature = "validator", validate(length(min = 0, max = 8000)))]
        pub content: Option<String>,

        /// Tag ids
        pub tags: Option<Vec<String>>,

        /// Attachment ids to add
        ///
        /// Appends: editing to paste an image must not drop what was already
        /// attached.
        pub attachments: Option<Vec<String>>,

        /// Whether the post is pinned
        pub pinned: Option<bool>,

        /// Whether the post is locked
        pub locked: Option<bool>,

        /// Fields to remove
        pub remove: Option<Vec<FieldsForumPost>>,
    }

    /// Options when querying a forum feed
    #[cfg_attr(feature = "rocket", derive(rocket::FromForm))]
    #[cfg_attr(feature = "validator", derive(validator::Validate))]
    pub struct OptionsQueryForumPosts {
        /// How to order the feed; defaults to the channel's configured sort
        pub sort: Option<super::ForumSort>,

        /// Only return posts carrying this tag
        pub tag: Option<String>,

        /// Maximum number of posts to return
        #[cfg_attr(feature = "validator", validate(range(min = 1, max = 100)))]
        pub limit: Option<i64>,

        /// Return posts that come after this one in the current ordering
        #[cfg_attr(feature = "validator", validate(length(min = 26, max = 26)))]
        pub after: Option<String>,
    }

    /// Response when fetching a forum feed
    pub struct ForumPostsResponse {
        /// The posts, in the requested order
        pub posts: Vec<ForumPost>,

        /// Authors of the posts
        pub users: Vec<super::User>,

        /// Server members for the authors
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        pub members: Option<Vec<super::Member>>,
    }

    /// Edit the configuration of a forum channel
    #[cfg_attr(feature = "validator", derive(validator::Validate))]
    pub struct DataEditForumChannel {
        /// Tags a post may carry
        #[cfg_attr(feature = "validator", validate(length(max = 32)))]
        pub available_tags: Option<Vec<super::ForumTag>>,

        /// Default ordering of the feed
        pub default_sort: Option<super::ForumSort>,

        /// Whether a post must carry at least one tag
        pub require_tag: Option<bool>,
    }

    /// A comment on a forum post
    pub struct ForumComment {
        /// Unique Id
        #[cfg_attr(feature = "serde", serde(rename = "_id"))]
        pub id: String,

        /// Id of the post this comment belongs to
        pub post: String,
        /// Id of the forum channel
        pub channel: String,
        /// Id of the user who wrote this comment
        pub author: String,

        /// Comment body, markdown
        pub content: String,

        /// Attached files
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        pub attachments: Option<Vec<File>>,

        /// Id of the comment being replied to
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        pub parent: Option<String>,

        /// Ids from the root down to the parent
        #[cfg_attr(feature = "serde", serde(default))]
        pub ancestors: Vec<String>,

        /// Ids of users who upvoted
        #[cfg_attr(feature = "serde", serde(default))]
        pub upvoters: Vec<String>,

        /// Number of upvotes
        #[cfg_attr(feature = "serde", serde(default))]
        pub score: i32,

        /// When this comment was last edited
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        pub edited: Option<String>,

        /// Id of whoever deleted this comment
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        pub deleted_by: Option<String>,
    }

    /// How a comment thread is ordered
    #[cfg_attr(feature = "rocket", derive(rocket::FromFormField))]
    pub enum ForumCommentSort {
        /// Highest score first
        Top,
        /// Oldest first
        Old,
        /// Newest first
        New,
    }

    /// Create a comment on a forum post
    #[cfg_attr(feature = "validator", derive(validator::Validate))]
    pub struct DataCreateForumComment {
        /// Comment body
        #[cfg_attr(feature = "validator", validate(length(min = 1, max = 8000)))]
        pub content: String,

        /// Id of the comment being replied to; omit for a top level comment
        #[cfg_attr(feature = "validator", validate(length(min = 26, max = 26)))]
        pub parent: Option<String>,

        /// Attachment ids
        pub attachments: Option<Vec<String>>,
    }

    /// Edit a forum comment
    #[cfg_attr(feature = "validator", derive(validator::Validate))]
    pub struct DataEditForumComment {
        /// Comment body
        #[cfg_attr(feature = "validator", validate(length(min = 1, max = 8000)))]
        pub content: Option<String>,

        /// Attachment ids to add
        pub attachments: Option<Vec<String>>,

        /// Fields to remove
        pub remove: Option<Vec<FieldsForumComment>>,
    }

    /// Fields that can be removed from a forum comment
    pub enum FieldsForumComment {
        Attachments,
    }

    /// Options when querying a comment thread
    #[cfg_attr(feature = "rocket", derive(rocket::FromForm))]
    pub struct OptionsQueryForumComments {
        /// How to order the thread
        pub sort: Option<ForumCommentSort>,
    }

    /// Response when fetching a comment thread
    pub struct ForumCommentsResponse {
        /// Every comment on the post, flat. The client assembles the tree
        /// from `ancestors`.
        pub comments: Vec<ForumComment>,

        /// Authors of the comments
        pub users: Vec<super::User>,

        /// Server members for the authors
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        pub members: Option<Vec<super::Member>>,
    }
);
