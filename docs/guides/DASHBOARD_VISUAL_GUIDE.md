# Dashboard Visual Guide

## Tab Overview

### Tab 1: Overview

```
┌─ 🚀 Cardano Node Dashboard ─────────────────────────────────────┐
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
┌─ Tabs ───────────────────────────────────────────────────────────┐
│ [Overview] Stake Pool | Wallets | Network | Monitoring | ...    │
└──────────────────────────────────────────────────────────────────┘
┌─ Content ────────────────────────────────────────────────────────┐
│ Syncing: [████████████████████░░░░] 89.5%                        │
│                                                                  │
│ Uptime: 2 days, 5 hours                                          │
│                                                                  │
│ ┌─ Node Statistics ────────────────────────────────────────┐    │
│ │ Chain Tip          │ 10,234,567                           │    │
│ │ Sync Progress      │ 89.5%                                │    │
│ │ Peer Count         │ 18                                   │    │
│ │ Transactions       │ 1,523,456                            │    │
│ │ CPU Usage          │ 28.3%                                │    │
│ │ Memory             │ 2.1 GB                               │    │
│ │ Network In         │ 125.3 MB/s                           │    │
│ │ Network Out        │ 89.7 MB/s                            │    │
│ └──────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────┘
┌─ Status ─────────────────────────────────────────────────────────┐
│ Press [q] to quit | [1-8] to switch tabs | [?] for help         │
└──────────────────────────────────────────────────────────────────┘
```

### Tab 2: Stake Pool

```
┌─ Tabs ───────────────────────────────────────────────────────────┐
│ Overview [Stake Pool] Wallets | Network | Monitoring | ...      │
└──────────────────────────────────────────────────────────────────┘
┌─ Stake Pool Information ─────────────────────────────────────────┐
│ Pool ID: pool1abc123...xyz789                                    │
│ Ticker: RUST | Name: Rust Cardano Pool                           │
│ Status: Active                                                   │
│ Pledge: 2,000,000 ₳ | Margin: 2.00% | Fixed Cost: 340 ₳         │
│ Active Stake: 15,000,000 ₳ (45.32% saturation)                   │
│ Delegators: 1,247 | Rank: #42                                    │
└──────────────────────────────────────────────────────────────────┘
┌─ Performance Metrics ────────────────────────────────────────────┐
│ Metric          │ Current Epoch      │ Lifetime                 │
│ ────────────────┼────────────────────┼─────────────────────────│
│ Blocks Minted   │ 23 / 25 expected   │ 4,567                    │
│ ROA             │ 4.23%              │ N/A                      │
│ Performance     │ 92.0%              │ N/A                      │
└──────────────────────────────────────────────────────────────────┘
```

### Tab 3: Wallets

```
┌─ Tabs ───────────────────────────────────────────────────────────┐
│ Overview | Stake Pool [Wallets] Network | Monitoring | ...      │
└──────────────────────────────────────────────────────────────────┘
┌─ Wallets ────────────────┐┌─ Wallet Details ────────────────────┐
│ Main Wallet - 1,500 ₳    ││ Name: Main Wallet                   │
│ Rewards Wallet - 250 ₳   ││ Address: addr1qxxx...xxxxx          │
│                          ││ Balance: 1,500 ₳                    │
│                          ││ Staked: 1,000 ₳                     │
│                          ││ Rewards: 50 ₳                       │
│                          ││ Delegated to: RUST                  │
│                          ││ UTxO Count: 25                      │
│                          ││ Transactions: 150                   │
│                          ││                                     │
│                          ││ Actions:                            │
│                          ││   [d] Delegate stake                │
│                          ││   [w] Withdraw rewards              │
│                          ││   [s] Send ADA                      │
└──────────────────────────┘└─────────────────────────────────────┘
```

### Tab 4: Network

```
┌─ Tabs ───────────────────────────────────────────────────────────┐
│ Overview | Stake Pool | Wallets [Network] Monitoring | ...      │
└──────────────────────────────────────────────────────────────────┘
┌─ Network Peers ──────────────────────────────────────────────────┐
│ Address                  │ Status    │ Protocol │ Sync %         │
│ ────────────────────────┼───────────┼──────────┼───────────────│
│ 123.45.67.89:3001        │ Connected │ 8.7.3    │ 100%           │
│ 234.56.78.90:3001        │ Connected │ 8.7.3    │ 100%           │
│ 45.67.89.12:3001         │ Connected │ 8.7.2    │ 98%            │
│ ...                      │ ...       │ ...      │ ...            │
└──────────────────────────────────────────────────────────────────┘
```

### Tab 5: Monitoring

```
┌─ Tabs ───────────────────────────────────────────────────────────┐
│ Overview | ... | Network [Monitoring] API Settings | ...        │
└──────────────────────────────────────────────────────────────────┘
┌─ CPU Usage ──────────────┐┌─ Memory Usage (GB) ─────────────────┐
│ 100% ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀││ 8GB ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀│
│  50% ⠀⢀⣠⣤⣤⣀⠀⠀⣀⣤⣄⠀⠀⠀⣠⣤⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀││ 4GB ⠀⢀⣀⣀⣀⡀⠀⠀⣀⣀⡀⠀⠀⢀⣀⣀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀│
│   0% ⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀││ 0GB ⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀│
│      0────────30────────60││      0────────30────────60         │
└──────────────────────────┘└─────────────────────────────────────┘
┌─ System Metrics ─────────────────────────────────────────────────┐
│ Metric      │ Current        │ Average                           │
│ ────────────┼────────────────┼──────────────────────────────────│
│ CPU         │ 28.3%          │ 25.7%                             │
│ Memory      │ 2.15 GB        │ 2.08 GB                           │
│ Network In  │ 125.30 MB/s    │ N/A                               │
│ Network Out │ 89.70 MB/s     │ N/A                               │
└──────────────────────────────────────────────────────────────────┘
```

### Tab 6: API Settings

```
┌─ Tabs ───────────────────────────────────────────────────────────┐
│ Overview | ... | Monitoring [API Settings] Alerts | Logs        │
└──────────────────────────────────────────────────────────────────┘
┌─ API Settings ───────────────────────────────────────────────────┐
│ API Configuration                                                │
│                                                                  │
│ REST API:       ✓ Enabled (Port: 8080)                           │
│ WebSocket:      ✓ Enabled (Port: 8081)                           │
│ Prometheus:     ✓ Enabled (Port: 12798)                          │
│ EKG:            ✓ Enabled (Port: 12788)                           │
│                                                                  │
│ Authentication: ✗ Not Required                                   │
│ Rate Limiting:  100 req/min                                      │
│                                                                  │
│ Configuration Options:                                           │
│   [r] Toggle REST API                                            │
│   [w] Toggle WebSocket                                           │
│   [p] Toggle Prometheus                                          │
│   [e] Toggle EKG                                                 │
│   [a] Toggle Authentication                                      │
│   [l] Set Rate Limit                                             │
└──────────────────────────────────────────────────────────────────┘
```

### Tab 7: Alerts

```
┌─ Tabs ───────────────────────────────────────────────────────────┐
│ Overview | ... | API Settings [Alerts] Logs                     │
└──────────────────────────────────────────────────────────────────┘
┌─ System Alerts (Press Enter to acknowledge) ─────────────────────┐
│ • [Critical] 14:23:45: Node sync stalled for 5 minutes           │
│ ✓ [Warning] 14:15:32: High memory usage detected (85%)           │
│ • [Error] 14:10:18: Peer connection lost: 123.45.67.89           │
│ ✓ [Info] 14:05:00: New block received                            │
│ • [Warning] 13:58:12: Disk space below 20%                       │
│ ✓ [Info] 13:45:23: Pool delegation received                      │
│                                                                  │
│ (Use ↑/↓ to navigate, Enter to acknowledge)                      │
└──────────────────────────────────────────────────────────────────┘
```

### Tab 8: Logs

```
┌─ Tabs ───────────────────────────────────────────────────────────┐
│ Overview | ... | Alerts [Logs]                                  │
└──────────────────────────────────────────────────────────────────┘
┌─ Node Logs ──────────────────────────────────────────────────────┐
│ [14:25:34] Block validated successfully                          │
│ [14:25:28] New peer connected                                    │
│ [14:25:21] Transaction added to mempool                          │
│ [14:25:15] Sync progress updated                                 │
│ [14:25:09] Block validated successfully                          │
│ [14:25:02] Transaction added to mempool                          │
│ [14:24:56] New peer connected                                    │
│ [14:24:50] Sync progress updated                                 │
│ [14:24:43] Block validated successfully                          │
│ ...                                                              │
└──────────────────────────────────────────────────────────────────┘
```

## Help Modal

Press **[?]** to display:

```
╔════════════════════════════════════════════════════════╗
║        Cardano Node Dashboard - Help                  ║
╚════════════════════════════════════════════════════════╝

Navigation:
  [1-8]         - Switch to specific tab
  [Tab]         - Next tab
  [Shift+Tab]   - Previous tab
  [↑/↓]         - Navigate items in lists
  [Enter]       - Select/Acknowledge item

Commands:
  [:]           - Enter command mode
  [:quit or :q] - Quit dashboard
  [:help or :h] - Show this help
  [:clear]      - Clear logs

Tabs:
  1. Overview      - Node status and blockchain info
  2. Stake Pool    - Pool operations and statistics
  3. Wallets       - Wallet management and balances
  4. Network       - Peer connections and topology
  5. Monitoring    - Performance metrics and graphs
  6. API Settings  - Configure REST/WebSocket/Prometheus
  7. Alerts        - System alerts and notifications
  8. Logs          - Node activity logs

Press [q] or [Esc] to close this help
```

## Command Mode

Press **[:]** to enter command mode:

```
┌─ Status ─────────────────────────────────────────────────────────┐
│ : quit                                                           │
└──────────────────────────────────────────────────────────────────┘
```

Available commands:

- `:quit` or `:q` - Exit dashboard
- `:help` or `:h` - Show help modal
- `:clear` - Clear log entries

## Color Legend

- **Cyan**: Titles, informational items, CPU graphs
- **Yellow**: Selected tabs, highlighted items, warnings
- **Green**: Success states, memory graphs
- **Red**: Critical alerts, errors, quit action
- **Light Red**: Error alerts
- **Gray**: Logs, acknowledged items
- **White**: Default text

## Navigation Flow

```
                    Tab Navigation
    [1]          [2]           [3]          [4]
  Overview  →  Stake Pool  →  Wallets  →  Network
     ↑                                         ↓
  [Logs]                                  [Monitoring]
     ↑                                         ↓
  [Alerts]    ←  API Settings  ←  [Monitoring]
    [7]             [6]              [5]
```

## Interactive Elements

### Wallets Tab

- **Left Panel**: Selectable wallet list
  - `↑` / `↓` to navigate
  - Highlighted wallet shows in yellow
  - Shows balance preview

- **Right Panel**: Wallet details
  - Full information of selected wallet
  - Action menu for operations

### Alerts Tab

- Navigate with `↑` / `↓`
- Press `Enter` to acknowledge alert
- Acknowledged alerts turn gray with ✓ prefix
- Unacknowledged alerts show • prefix
- Color-coded by severity

### Monitoring Tab

- **Graphs**: Real-time updating
  - CPU: 0-100% scale
  - Memory: 0-8GB scale
  - 60 historical samples
  - Braille line rendering

- **Metrics Table**: Live statistics
  - Current values
  - Running averages
  - Network throughput

## Status Bar States

### Normal Mode

```
Press [q] to quit | [1-8] to switch tabs | [?] for help
Connected: Yes | Syncing: 89.5%
```

### Command Mode

```
: <your command here>
```

### Editing Mode (future)

```
EDIT: <your input here>
```

## Keyboard Shortcuts Reference

| Key | Action |
|-----|--------|
| `1` | Go to Overview tab |
| `2` | Go to Stake Pool tab |
| `3` | Go to Wallets tab |
| `4` | Go to Network tab |
| `5` | Go to Monitoring tab |
| `6` | Go to API Settings tab |
| `7` | Go to Alerts tab |
| `8` | Go to Logs tab |
| `Tab` | Next tab |
| `Shift+Tab` | Previous tab |
| `↑` | Move up in list |
| `↓` | Move down in list |
| `Enter` | Select/Acknowledge |
| `:` | Command mode |
| `?` | Toggle help |
| `q` | Quit (normal mode) |
| `Esc` | Cancel/Close |

## Usage Examples

### Quick Navigation

1. Press `2` to jump to Stake Pool tab
2. Press `Tab` twice to reach Network tab
3. Press `5` to see Monitoring graphs

### Managing Wallets

1. Press `3` for Wallets tab
2. Use `↑` / `↓` to select wallet
3. View details in right panel
4. (Future) Press action keys for operations

### Viewing Alerts

1. Press `7` for Alerts tab
2. Navigate to critical alert with `↑` / `↓`
3. Press `Enter` to acknowledge
4. Alert turns gray with ✓

### Getting Help

1. Press `?` from any tab
2. Read comprehensive help
3. Press `q` or `Esc` to close

### Quitting

- Method 1: Press `q` from normal mode
- Method 2: Press `:` then type `quit` and `Enter`
- Method 3: Press `Esc` to cancel, then `q`
