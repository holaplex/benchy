pub use csv::Writer;
use serde::Serialize;
use uuid::Uuid;
#[derive(Debug, Serialize)]
pub struct Record {
    pub mint_id: Uuid,
    pub completion_sec: u64,
    pub retry_count: u64,
    pub success: bool,
}
