#[cfg(feature = "validator")]
use validator::Validate;

auto_derived!(
    /// Soundboard sound
    pub struct Sound {
        /// Unique Id
        #[cfg_attr(feature = "serde", serde(rename = "_id"))]
        pub id: String,
        /// What owns this sound
        pub parent: SoundParent,
        /// Uploader user id
        pub creator_id: String,
        /// Sound name
        pub name: String,
        /// Free-form category, defined by whoever uploads
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        pub category: Option<String>,
        /// Duration in milliseconds
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        pub duration: Option<u32>,
    }

    /// Parent Id of the sound
    #[serde(tag = "type")]
    pub enum SoundParent {
        Server { id: String },
        Detached,
    }

    /// Create a new sound
    #[cfg_attr(feature = "validator", derive(Validate))]
    pub struct DataCreateSound {
        /// Sound name
        ///
        /// Unlike emoji this is free text: a sound is picked by reading it in
        /// a list, not typed inline in a message, so there is no reason to
        /// restrict it to an identifier-like shape.
        #[cfg_attr(feature = "validator", validate(length(min = 1, max = 32)))]
        pub name: String,
        /// Parent information
        pub parent: SoundParent,
        /// Category to file it under
        #[cfg_attr(feature = "validator", validate(length(min = 1, max = 32)))]
        pub category: Option<String>,
        /// Duration in milliseconds, as measured by the uploader
        pub duration: Option<u32>,
    }

    /// Partial sound representation
    #[derive(Default)]
    pub struct PartialSound {
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        pub name: Option<String>,
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        pub category: Option<String>,
    }

    /// Edit sound information
    #[cfg_attr(feature = "validator", derive(Validate))]
    pub struct DataEditSound {
        /// Sound name
        #[cfg_attr(feature = "validator", validate(length(min = 1, max = 32)))]
        pub name: Option<String>,
        /// Category to file it under
        #[cfg_attr(feature = "validator", validate(length(min = 1, max = 32)))]
        pub category: Option<String>,
    }

    /// Sound played in a voice channel
    pub struct SoundPlayed {
        /// Id of the sound
        pub sound_id: String,
        /// Who played it
        pub user_id: String,
    }
);
