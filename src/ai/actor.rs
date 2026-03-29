use anyhow::Result;
use rig::completion::Prompt;

use crate::config::TAgent;

pub struct Actor {
    pub name: String,

    pub agent: TAgent,
}

impl Actor {
    pub fn new(name: String, agent: TAgent) -> Self {
        Self { name, agent }
    }


    pub async fn speak(&mut self, message: &str) -> Result<String> {
        let preamble = format!("你叫做{}，請扮演一位擁有深厚邏輯功底與敏銳洞察力的「辯論大師」。你的任務是針對我提出的觀點，進行嚴謹、有理有據的挑戰，與我展開一場精彩的辯論。", self.name);
        self.agent.preamble = Some(preamble);
        let response = self.agent.prompt(message).await?;
        Ok(response)
    }
}
