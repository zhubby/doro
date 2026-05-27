use doro_protocol::EnrollmentToken;
use uuid::Uuid;

pub fn generate_enrollment_token(label: impl Into<String>) -> EnrollmentToken {
    EnrollmentToken {
        id: Uuid::new_v4(),
        label: label.into(),
    }
}
