use crate::config::TAgent;

pub struct Director {
    pub agent: TAgent,
}

impl Director {
    pub fn new(agent: TAgent) -> Self {
        Self { agent }
    }
}
