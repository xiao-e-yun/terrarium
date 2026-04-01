use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    sync::Arc,
};

use anyhow::{Result, bail};
use dashmap::DashSet;
use rand::seq::{IndexedRandom, SliceRandom};
use rig::{
    completion::{Chat, Prompt},
    message::Message,
};
use serde::Deserialize;
use tokio::sync::{Mutex, broadcast};
use tracing::{error, warn};

use crate::{config::TAgent, role::Role};

use super::{context::Context, director::Director};

#[derive(Clone)]
pub struct Actor {
    // Readonly fields
    role: Role,
    name: String,

    personality: String,

    shared: Arc<(DashSet<ActorStatus>, Mutex<ActorAgent>)>,
}

impl Actor {
    pub fn role(&self) -> Role {
        self.role
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn personality(&self) -> &String {
        &self.personality
    }

    pub fn status(&self) -> &DashSet<ActorStatus> {
        &self.shared.as_ref().0
    }

    pub fn agent(&self) -> &Mutex<ActorAgent> {
        &self.shared.as_ref().1
    }

    pub async fn generate_many(director: &Director, count: u8) -> Result<Vec<Self>> {
        if count < 3 {
            bail!("Too few");
        };

        let roles = Self::generate_roles(count);

        let names = Self::generate_names(count);

        let mut personalities = vec![];
        while personalities.len() < count as usize {
            personalities.extend(Self::generate_personality(&director.agent).await?);
        }

        let mut actors = Vec::new();
        for ((role, name), personality) in roles.into_iter().zip(names).zip(personalities) {
            let mut agent = director.agent.clone();
            agent.name = Some(name.clone());
            let context = Context::new(format!("你是 \"{}\"

[嚴格限制]
按照你的角色設定行動，並且在任何情況下都不能違反你的角色設定。你必須完全融入角色，並且在任何情況下都不能透露你是個 AI 或者打破第四面牆。

[身分與目標]
你的身分是\"{}\"，
{}

[人格設定]
{}", &name, role, role.description(), personality));
            context.bind(&mut agent);

            actors.push(Actor {
                role,
                name,
                personality,
                shared: Arc::new((
                    DashSet::new(),
                    Mutex::new(ActorAgent {
                        context,
                        history: vec![],
                        broadcast: HashMap::new(),
                        agent,
                    }),
                )),
            });
        }
        Ok(actors)
    }

    async fn generate_personality(agent: &TAgent) -> Result<Vec<String>> {
        let mut err = None;
        for attempt in 1..=3 {
            let personalities = agent
                .prompt("請生成 5 組常見但偶有亮點的「一般性人格」設定。

[嚴格限制]
1. 字數限制: 每組設定的描述必須控制在 20 字以內，精煉且清晰易懂。
2. 輸出格式: 只能輸出合法的 JSON 格式 `string[]`，絕對不准包含任何解釋、問候語或 Markdown 標記(如 ```json)。
3. 合理性與風格: 大多數設定應具備平凡、生活感的普通特徵，但允許少數（約1至2組）帶有較為鮮明、有趣的個人怪癖或特色，以增添趣味。

[輸出範例]
[\"熱心但過度嘮叨的社區管理員\", \"總是害怕扛責的疲憊上班族\", \"溫和內向、熱愛閱讀的大學生\", \"極度強迫症，看見東西歪掉會崩潰\", \"熱衷於收集陌生人雨傘的神秘老人\"]")
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

    fn generate_names(count: u8) -> Vec<String> {
        let family_names = [
            "陳", "林", "李", "王", "張", "劉", "黃", "楊", "吳", "趙", "周", "徐", "孫", "馬",
            "朱", "胡", "郭", "何", "高", "羅", "鄭", "梁", "謝", "韓", "唐", "馮", "宋", "程",
            "曹", "彭",
        ];

        let given_names = [
            "冠宇", "欣怡", "志豪", "雅婷", "俊宏", "佩君", "承恩", "詠晴", "宇軒", "婉婷", "嘉宏",
            "淑芬", "建廷", "美玲", "柏翰", "慧君", "子軒", "鈺婷", "冠廷", "雅玲", "智傑", "詩涵",
            "哲瑋", "曉薇", "郁翔", "曼婷", "育誠", "佳蓉", "宗翰", "宜蓁", "祥宇", "芷萱", "振宇",
            "惠雯", "家誠", "雅雯", "宏明", "雅芳", "志強", "郁婷",
        ];

        let mut names = vec![];
        let mut rng = rand::rng();
        while names.len() < count as usize {
            let f = family_names.choose(&mut rng).unwrap();
            let g = given_names.choose(&mut rng).unwrap();
            if !names.contains(&format!("{}{}", f, g)) {
                names.push(format!("{}{}", f, g));
            }
        }

        names
    }

    fn generate_roles(count: u8) -> Vec<Role> {
        let mut roles = vec![Role::Innocent; count as usize];
        roles[0] = Role::Murderer;
        roles[1] = Role::Sheriff;

        let mut rng = rand::rng();
        roles.shuffle(&mut rng);

        roles
    }
}

impl PartialEq for Actor {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }
}

impl Debug for Actor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Actor {{ name: {}, role: {}, personality: {} }}",
            self.name, self.role, self.personality
        )
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ActorStatus {
    Dead,
}

impl Display for ActorStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActorStatus::Dead => write!(f, "死亡"),
        }
    }
}

pub struct ActorAgent {
    pub context: Context,
    pub history: Vec<Message>,
    pub broadcast: HashMap<&'static str, broadcast::Receiver<Message>>,

    pub agent: TAgent,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ActorAction {
    pub action: String,
    pub content: String,
    pub thought: String,
}

impl ActorAgent {
    /// sync broadcast and context
    pub fn sync(&mut self) -> Result<()> {
        for rx in self.broadcast.values_mut() {
            while let Ok(msg) = rx.try_recv() {
                self.history.push(msg);
            }
        }
        self.context.bind(&mut self.agent);
        Ok(())
    }
    pub async fn action(&mut self, prompt: &str) -> Result<ActorAction> {
        let prompt = Message::system(format!(
            "請你嚴格遵守以下輸出格式規則，**絕對不要**輸出任何額外的解釋、問候語或在 JSON 範圍外的思考過程。必須確保輸出為純粹且合法的 JSON 格式。

### [輸出規則]
嚴格按照以下 JSON 結構輸出，每次只能輸出單一個 JSON 物件，代表一個動作。該物件必須包含 `thought`、`action` 和 `content` 三個鍵值。任何不符合此 JSON 格式的輸出都將被視為無效。

### [JSON 欄位定義]
* **`thought`**：字串。第一步的思考過程，如果沒有思考過程則填入空字串 `\"\"`。
* **`action`**：字串。行為名稱，必須完全匹配。
* **`content`**：字串。根據行為名稱的不同而有所區別，若無對應內容則填入空字串 `\"\"`。

{}",
            prompt
        ));

        for _ in 1..=5 {
            let response = match self.agent.chat(prompt.clone(), self.history.clone()).await {
                Ok(res) => res,
                Err(e) => {
                    error!("Failed to get response: {}", e);
                    continue;
                }
            };

            match serde_json::from_str(&response) {
                Ok(r) => {
                    self.history.push(Message::assistant(response.clone()));
                    return Ok(r)
                },
                Err(_) => warn!("Failed to parse response: {}", response),
            }
        }
        bail!("Failed to get valid action after 3 attempts");
    }
}
