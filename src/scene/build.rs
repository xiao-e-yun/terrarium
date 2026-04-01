use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use rig::completion::Prompt;
use tokio::sync::broadcast;
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
    let prompt = format!("根據一場景描述，生成包含多個區域的地圖結構，並用一句話描述每個區域的特點和功能

[嚴格限制]
1. 輸出格式: 多行文本，每行描述一個區域，格式為\"區域名稱:一句話描述\"，其中區域名稱應簡潔明了。
2. 零廢話: 直接給出結果，嚴禁輸出任何解釋、問候語或額外的文字。
3. 字數限制: 每行描述的字數不超過 30 字，且整體描述應清晰易懂，能夠幫助玩家快速理解各區域的特點和用途。
4. 內容要求: 描述應涵蓋區域的主要功能、氛圍，不得包含任何明確物品。
5. 區域數量: 至少需要包含 8 個區域。

[輸出範例]
客廳: 一個寬敞的空間，擺放著破舊的沙發和一台老式電視
廚房: 充滿了各種烹飪工具和食材的地方
走廊: 連接各個房間的狹長通道，牆上掛滿了古老的畫作
一號臥室: 一個昏暗的房間，裡面有一張破舊的床和一個衣櫃
二號臥室: 一個明亮的房間，裡面有一張整潔的床和一個書桌
...

[場景描述]
{}", concept);

    fn parse_rooms(input: &str) -> Result<Vec<Room>> {
        input
            .lines()
            .map(|line| {
                let line = line.replace(':', "：");
                let (name, description) = line
                    .split_once('：')
                    .ok_or(anyhow::anyhow!("Invalid room format: {}", line))?;
                let broadcast = broadcast::Sender::new(128);
                Ok(Room {
                    name: name.trim().to_string(),
                    description: description.trim().to_string(),
                    actors: vec![],
                    broadcast,
                })
            })
            .try_collect()
    }

    let mut err = None;
    for attempt in 1..=3 {
        let rooms = director.agent.prompt(&prompt).await?;
        match parse_rooms(&rooms) {
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
                err = Some(e);
            }
        }
    }

    Err(err.unwrap())?
}
