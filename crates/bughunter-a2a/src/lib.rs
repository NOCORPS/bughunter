pub mod router;
pub mod types;

pub use router::a2a_router;
pub use types::{
    A2aTaskRequest, A2aTaskResponse, AgentCard, AgentCardCapabilities, AgentCardSkill,
};
