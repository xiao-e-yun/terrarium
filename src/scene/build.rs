use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use rig::completion::Prompt;
use tracing::{debug, error};

use crate::{ai::director::Director, scene::room::Room};

use super::Scene;

pub async fn generate(director: &Director) -> Result<Scene> {
    debug!("Generating scene...");

    debug!("Generating scene concept");
    let prompt = "設計一個場景並用一句話描述

[嚴格限制]
1. 字數與格式:只能輸出「一句」結構完整的中文，包含標點符號絕對不可超過 40 個字。
2. 零廢話:直接給出結果，嚴禁輸出任何解釋、問候語、引號或額外的文字。
3. 空間層次:描述的地點必須明確呈現「多個區域」或「內外空間對比」（例如:車廂與車外、大廳與走廊、甲板與船艙）。
4. 可存活:存活 48 小時後，可以逃出去的場景。(不得明確描述逃生路徑或方法)
5. 可玩性:場景應該具有足夠的現實感，讓玩家能夠在其中進行探索和互動，並且具有一定的挑戰性和緊張感。
6. 主次分明:場景不應該超出遊戲的規則和限制，應該能夠為玩家提供一個清晰的遊戲環境和背景故事，並且能夠引導玩家進行遊戲。

[輸出範例]
一趟的長途火車，車廂內充斥著緊張的氣氛和不安的乘客。
困於暴風雪中的山間別墅，內部溫暖但外部風雪交加，讓人感到壓抑和孤立。
一艘在暴風雨中航行的破舊船隻，甲板上滿是損壞的設備和緊張的船員。";
    let concept = director.agent.prompt(prompt).await?;

    let rooms = generate_rooms(director, &concept).await?;

    Ok(Scene {
        description: concept,
        rooms,
    })
}

async fn generate_rooms(director: &Director, concept: &str) -> Result<BTreeMap<String, Room>> {
    let prompt = format!("根據場景描述，生成包含多個區域的地圖結構，並為每個區域設計相應的物品及特點描述。

### [嚴格限制]
1. **輸出格式**: 必須輸出一個純淨且合法的壓縮後的 JSON 陣列 (Array)，陣列中的每個物件代表一個區域，並嚴格遵守指定的資料結構。
2. **零廢話**: 直接給出 JSON 結果，**嚴禁**輸出任何解釋、問候語或 JSON 範圍外的額外文字。
3. **字數限制**: 每個區域的 `description` 字數不超過 30 字，應清晰易懂，幫助玩家快速理解各區域的特點和用途。
4. **物品要求**: 每個區域內的 `items` 清單必須合理分配物品。物品的 `tags` 標籤**必須且只能**從 `\"drink\"`、`\"food\"`、`\"weapon\"`` 這三種中選擇（可包含一個或多個標籤），物品的 `description` 描述必須說明包含什麼功能(可食用、可作為武器)。
5. **物品數量**: 食物和飲料類物品的數量應該適中，既要提供足夠的資源讓玩家能夠生存下去，又要保持一定的挑戰性。武器類物品的數量應該有限，以增加遊戲的緊張感和策略性。
6. **區域數量**: 輸出的 JSON 陣列中至少需要包含 8 個區域。

### [JSON 資料結構要求]
每一個區域物件必須嚴格符合以下格式：
```json
{{
  \"name\": \"字串 (簡潔明瞭的區域名稱)\",
  \"items\": [
    {{
      \"name\": \"字串 (物品名稱)\",
      \"description\": \"字串 (物品的簡短描述)\",
      \"tags\": [\"drink\", \"food\", \"weapon\"] 
    }}
  ],
  \"description\": \"字串 (一句話描述該區域的特點與功能)\"
}}
```

### [輸出範例]
```json
[
  {{
    \"name\": \"廚房\",
    \"items\": [
      {{
        \"name\": \"生鏽的菜刀\",
        \"description\": \"刀刃已經嚴重生鏽，但依然可以用來防身。\",
        \"tags\": [\"weapon\"]
      }},
      {{
        \"name\": \"半瓶純水\",
        \"description\": \"剩下半瓶的乾淨飲用水。\",
        \"tags\": [\"drink\"]
      }}
    ],
    \"description\": \"充滿了各種烹飪工具和殘留食材的料理空間\"
  }},
  {{
    \"name\": \"客廳\",
    \"items\": [
      {{
        \"name\": \"棒球棍\",
        \"description\": \"靠在沙發角落的木製棒球棍。\",
        \"tags\": [\"weapon\"]
      }}
    ],
    \"description\": \"一個寬敞的空間，擺放著破舊的沙發和一台老式電視\"
  }}
]
```

### [場景描述]
{}", concept);

    let mut err = None;
    for attempt in 1..=3 {
        let rooms = director.agent.prompt(&prompt).await?;
        match serde_json::from_str::<Vec<Room>>(&rooms) {
            Ok(list) => {
                if list.len() >= 8 {
                    return Ok(list
                        .into_iter()
                        .map(|room| (room.name.clone(), room))
                        .collect());
                } else {
                    err = Some(anyhow!("Too few room"));
                }
            }
            Err(e) => {
                error!("Attempt {}: Failed to parse rooms: {}", attempt, e);
                err = Some(e.into());
            }
        }
    }

    Err(err.unwrap())?
}
