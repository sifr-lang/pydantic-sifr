mod callbacks;
mod json;
mod output;
mod plan;
mod selection;

pub use callbacks::{
    SerializationCallbacks, serialize_json_with_callbacks, serialize_structural_with_callbacks,
    serializer_callback_error,
};
pub use json::serialize_json;
pub use output::{SerializationError, SerializationErrorKind, serialize_structural};
pub use plan::{
    SerializationPlan, SerializationPlanError, SerializerCallbackKind, SerializerCallbackPlan,
    SerializerFieldPlan, SerializerNode, SerializerNodeId, SerializerWhenUsed,
};
pub use selection::{SelectionPath, SelectionSegment, SerializationOptions};
pub use sifr_runtime::json::{JsonIntegerProfile, JsonIntegerRangeError};
