# Terrarium

一款以 Rust 編寫、由本地 LLM 驅動的程序生成 AI 社交推理遊戲。觀看 AI 角色在動態生成的環境中進行生存、欺騙與謀殺的戲劇。

## 功能特色

- **程序生成**：每次遊戲的場景、房間、物品與角色個性均由 AI 動態生成
- **AI 驅動角色**：每個角色是獨立的 LLM 代理，擁有獨特個性、目標與持久記憶
- **社交推理機制**：三種角色（殺手、警長、平民），各有不同的勝利條件
- **生存系統**：飢餓與口渴值每回合遞減，歸零即死亡
- **動態互動**：物品管理、房間行動與行動後對話階段
- **本地 LLM**：透過 Ollama 運行，無需 API 金鑰

## 環境需求

- **Rust** 1.70+
- **Ollama**（本地運行）

## 安裝與執行

### 1. 安裝 Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. 安裝 Ollama 並拉取模型

前往 [https://ollama.com](https://ollama.com) 下載安裝，然後：

```bash
ollama pull isotnek/qwen3.5:9B-Unsloth-UD-Q4_K_XL
```

### 3. 建置並執行

```bash
git clone https://github.com/xiaoeyun/terrarium.git
cd terrarium
cargo run --release
```

首次執行會自動產生 `config.toml`，接著生成場景與角色，遊戲即開始。

## 遊戲說明

### 遊戲流程

模擬時間共 **24 小時**（288 回合，每回合 5 分鐘）。每回合依序進行：

1. **行動階段**：AI 代理決定並執行行動
2. **對話階段**：同房間的角色決定是否發言
3. **時間推進**：遊戲內時間前進 5 分鐘

### 角色與勝利條件

| 角色 | 目標 |
|------|------|
| 殺手 | 在時限內殺死所有平民 |
| 警長 | 找出並消滅殺手 |
| 平民 | 存活並協助警長 |

### 可用行動

| 行動 | 說明 |
|------|------|
| `GOTO <房間>` | 移動到其他房間 |
| `OBSERVE` | 觀察當前房間 |
| `PICKUP <物品>` | 撿起物品 |
| `DROP <物品>` | 放下物品 |
| `USE <物品>` | 使用食物或飲料 |
| `ATTACK <目標>` | 攻擊角色（需持有武器） |
| `IDLE` | 原地等待 |

## 設定

`config.toml` 可自訂模型與伺服器位址：

```toml
model = "isotnek/qwen3.5:9B-Unsloth-UD-Q4_K_XL"
ollama_url = "http://localhost:11434"
```

更換模型只需 `ollama pull <模型名稱>` 後修改 `model` 欄位即可。

## 專案結構

```
src/
├── main.rs          # 遊戲循環與行動協調
├── config.rs        # Ollama 設定
├── role.rs          # 角色定義（殺手、警長、平民）
├── utils.rs         # UI 工具
├── ai/
│   ├── actor.rs     # 角色屬性、物品欄與 LLM 代理
│   ├── context.rs   # LLM 提示詞建構
│   └── director.rs  # 場景與角色生成導演 AI
└── scene/
    ├── mod.rs       # 場景管理
    ├── room.rs      # 房間與廣播系統
    ├── item.rs      # 物品定義
    └── build.rs     # 場景生成邏輯
```

## 常見問題

**Ollama 連線失敗**：確認 `ollama serve` 已在背景執行，並檢查 `config.toml` 中的 `ollama_url`。

**模型找不到**：執行 `ollama pull isotnek/qwen3.5:9B-Unsloth-UD-Q4_K_XL` 拉取模型。

**回應速度慢**：LLM 推理速度取決於硬體。可在 `config.toml` 換用較小的模型（如 7B）。

## 授權

MIT
