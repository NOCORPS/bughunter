pub mod adapters;
pub mod types;

pub use adapters::{AgentRegistry, CoreBackedAgentAdapter, RemoteProtocolAgentAdapter};
pub use types::{
    AgentAdapter, AgentCapabilities, AgentFrameworkKind, AgentProtocol, AgentTask, AgentTaskResult,
    AgentTaskState, AgentTaskStatus,
};
