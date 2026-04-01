use anyhow::Result;
use rand::seq::IndexedRandom;
use rig::completion::Prompt;
use tracing::{debug, error};

use crate::config::TAgent;

use super::context::Context;

pub struct Director {
    pub agent: TAgent,
    pub personality: String,
    pub context: Context,
}

impl Director {
    pub async fn new(mut agent: TAgent) -> Self {
        debug!("Generating director personality...");
        let personality = Self::random_personality(&agent).await.unwrap();

        let context = Context::new(format!(
            "你是一位遊戲敘事者。

[標誌性人格]
{}。

[遊戲規則]
一群人被困於此，必須等待救援或者找出並擊殺藏匿在他們之中的兇手。",
            personality
        ));
        context.bind(&mut agent);
        Self {
            agent,
            personality,
            context,
        }
    }

    pub async fn generate_personality(agent: &TAgent) -> Result<Vec<String>> {
        let mut err = None;
        for attempt in 1..=3 {
            let personalities = agent
                .prompt("請生成 3 組極具特色且鮮明的「標誌性人格」設定。

[嚴格限制]
1. 字數限制: 每行用大約 30-50 字的一句話精煉描述，且整體描述應清晰易懂。
2. 輸出格式: 只能輸出合法的 JSON 格式 `string[]`，絕對不准包含任何解釋、問候語或 Markdown 標記(如 ```json)。
3. 合理性: 設定應該具有一定的合理性和可行性，能夠在遊戲中被玩家理解和接受，並且能夠為遊戲增添深度和趣味性。
4. 個人風格: 每個設定都應該具有獨特的個人風格，能夠在遊戲中引發獨特的情節和衝突。例如，可以是極端的性格特徵、獨特的行為模式、特殊的信仰體系或異常的心理狀態等。

[輸出範例]
[\"極致的心理虐待狂，比起肉體傷害更愛看角色自相殘殺，熱衷於設計『只能活一個』的道德困境\", \"極度迷信的儀式執行者，所有事件的發生時間與死法都必須嚴格符合星象與中世紀的血腥獻祭規則\", \"患有嚴重潔癖的處刑人，無法忍受任何髒汙與血液噴濺，所有的陷阱都必須是高溫蒸發或絕對零度凍結\"]")
                .await?;
            match serde_json::from_str::<Vec<String>>(&personalities) {
                Ok(list) => return Ok(list),
                Err(e) => {
                    error!("Attempt {}: Failed to parse personalities: {}", attempt, e);
                    err = Some(e);
                }
            }
        }
        Err(err.unwrap())?
    }

    pub async fn random_personality(agent: &TAgent) -> Result<String> {
        let mut rng = rand::rng();
        let personalities = Self::generate_personality(agent).await?;
        Ok(personalities.choose(&mut rng).unwrap().to_string())
    }
}
