use std::fmt::{Display};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Murderer,
    Sheriff,
    Innocent,
}

impl Role {
    pub fn description(&self) -> &'static str {
        match self {
            Role::Murderer => "致死方休: 你必須在時限內肅清所有目擊者，將真相永遠掩埋。
獵人法則: 善用「移動 (GOTO)」尋找落單獵物，透過說話編織謊言。既然平民也能武裝自己，這便是你最好的掩護——挑撥離間，在混亂中「攻擊 (ATTACK)」並讓無辜者背負罪名。",

Role::Sheriff => "鐵腕制裁: 撥開謊言的迷霧，在局勢徹底失控前鎖定真兇，並行使你唯一的處決權。
破局之鑰: 頻繁「觀察 (OBSERVE)」蛛絲馬跡，透過說話盤問眾人。當平民也開始「拾取 (PICKUP)」武器自保時，你必須在恐慌與私刑蔓延前保持理智，果斷「攻擊 (ATTACK)」制裁真正的狼。",

Role::Innocent => "絕境求生: 在死亡陰影下盡力活下去。協助警長揪出兇手，或在必要時親手終結威脅。
生存代價: 積極「觀察 (OBSERVE)」並「拾取 (PICKUP)」武器以獲得「攻擊 (ATTACK)」的反擊能力。但手握武力將使你成為眾矢之的；謹慎透過說話證明清白，遇險則立刻「移動 (GOTO)」。別讓生存的渴望，使你淪為下一個施暴者。",
        }
    }
}

impl Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Murderer => write!(f,"殺手"),
            Role::Sheriff => write!(f,"警長"),
            Role::Innocent => write!(f,"平民"),
        }
    }
}
