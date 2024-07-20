use serde::{Deserialize, Serialize};

pub static CONFIG_QUALIFIER: &str = "moe";
pub static CONFIG_ORGANIZATION: &str = "Hamuko";
pub static CONFIG_APPLICATION: &str = "Beelzebub";

#[derive(Debug, Deserialize, Serialize)]
pub struct Submission {
    pub duration: u64,
    pub executable: String,
    pub name: Option<String>,
}

impl Submission {
    pub fn display(&self) -> String {
        let name = self.name.as_ref().unwrap_or(&self.executable);
        format!("{} ({}s)", name, &self.duration)
    }
}

#[derive(Serialize)]
pub enum SubmissionResponseStatus {
    DatabaseError,
    Ok,
    Unauthenticated,
}

#[derive(Serialize)]
pub struct SubmissionResponse {
    pub status: SubmissionResponseStatus,
}
