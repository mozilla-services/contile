//! Web authentication, handlers, and middleware
pub mod dockerflow;
pub mod extractors;
pub mod handlers;
pub mod middleware;
#[cfg(test)]
pub(crate) mod test;
mod user_agent;

pub use dockerflow::DOCKER_FLOW_ENDPOINTS;
pub use user_agent::{get_device_info, DeviceInfo, FormFactor, OsFamily};
