/// Interactive Terminal Dashboard for Cardano Node
///
/// Provides comprehensive real-time monitoring and management interface
use anyhow::Result;
use chrono::{DateTime, Local};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, Block, Borders, Cell, Chart, Clear, Dataset, Gauge, List, ListItem, Paragraph, Row,
        Table, Tabs, Wrap,
    },
    Frame, Terminal,
};
use std::{
    collections::VecDeque,
    io,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Dashboard state and data
pub struct Dashboard {
    /// Current selected tab
    selected_tab: usize,
    /// Current selected sub-tab for multi-tab sections (reserved for future use)
    #[allow(dead_code)]
    selected_subtab: usize,
    /// Current selected item in lists
    selected_item: usize,
    /// Node socket path (reserved for real node integration)
    #[allow(dead_code)]
    socket_path: Option<PathBuf>,
    /// Refresh interval
    refresh_interval: Duration,
    /// Last update time
    last_update: Instant,
    /// Node statistics
    stats: NodeStats,
    /// Stake pool information
    stake_pool: StakePoolInfo,
    /// Wallet information
    wallets: Vec<WalletInfo>,
    /// API settings
    api_settings: ApiSettings,
    /// Monitoring alerts
    alerts: Vec<Alert>,
    /// Log messages
    logs: Vec<String>,
    /// Historical metrics for graphs
    metrics_history: MetricsHistory,
    /// Input mode (for text input)
    input_mode: InputMode,
    /// Input buffer
    input_buffer: String,
    /// Should quit
    should_quit: bool,
    /// Show help modal
    show_help: bool,
}

#[derive(Default, Clone)]
pub struct NodeStats {
    pub chain_tip: u64,
    pub sync_progress: f64,
    pub total_blocks: u64,
    pub peer_count: usize,
    pub tx_processed: u64,
    pub mempool_size: usize,
    pub uptime: Duration,
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub disk_usage: u64,
    pub network_in: u64,
    pub network_out: u64,
    pub epoch: u64,
    pub slot: u64,
    pub blocks_forged: u64,
    pub active_stake: u64,
}

#[derive(Clone)]
pub struct StakePoolInfo {
    pub pool_id: String,
    pub ticker: String,
    pub name: String,
    pub pledge: u64,
    pub margin: f64,
    pub fixed_cost: u64,
    pub active_stake: u64,
    pub delegators: usize,
    pub blocks_minted: u64,
    pub blocks_expected: u64,
    pub lifetime_blocks: u64,
    pub saturation: f64,
    pub rank: usize,
    pub roa: f64, // Return on ADA
    pub status: PoolStatus,
}

#[derive(Clone, PartialEq, Debug)]
pub enum PoolStatus {
    Active,
    Retiring,
    Retired,
    Inactive,
}

#[derive(Clone)]
pub struct WalletInfo {
    pub name: String,
    pub address: String,
    pub balance: u64,
    pub staked: u64,
    pub rewards: u64,
    pub delegated_pool: Option<String>,
    pub utxo_count: usize,
    pub tx_count: usize,
}

#[derive(Clone)]
pub struct ApiSettings {
    pub rest_enabled: bool,
    pub rest_port: u16,
    pub websocket_enabled: bool,
    pub websocket_port: u16,
    pub prometheus_enabled: bool,
    pub prometheus_port: u16,
    pub ekg_enabled: bool,
    pub ekg_port: u16,
    pub auth_required: bool,
    pub rate_limit: u32,
}

#[derive(Clone)]
pub struct Alert {
    pub timestamp: DateTime<Local>,
    pub level: AlertLevel,
    pub message: String,
    pub acknowledged: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}

pub struct MetricsHistory {
    pub cpu: VecDeque<f32>,
    pub memory: VecDeque<u64>,
    pub network_in: VecDeque<u64>,
    pub network_out: VecDeque<u64>,
    pub blocks: VecDeque<u64>,
    pub max_samples: usize,
}

#[derive(PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
    Command,
}

impl StakePoolInfo {
    /// Create empty pool info - data should be loaded from node
    fn empty() -> Self {
        Self {
            pool_id: String::new(),
            ticker: String::new(),
            name: "No pool data available".to_string(),
            pledge: 0,
            margin: 0.0,
            fixed_cost: 0,
            active_stake: 0,
            delegators: 0,
            blocks_minted: 0,
            blocks_expected: 0,
            lifetime_blocks: 0,
            saturation: 0.0,
            rank: 0,
            roa: 0.0,
            status: PoolStatus::Retired,
        }
    }
}

impl Default for ApiSettings {
    fn default() -> Self {
        Self {
            rest_enabled: true,
            rest_port: 8080,
            websocket_enabled: true,
            websocket_port: 8081,
            prometheus_enabled: true,
            prometheus_port: 12798,
            ekg_enabled: true,
            ekg_port: 12788,
            auth_required: false,
            rate_limit: 100,
        }
    }
}

impl MetricsHistory {
    fn new(max_samples: usize) -> Self {
        Self {
            cpu: VecDeque::with_capacity(max_samples),
            memory: VecDeque::with_capacity(max_samples),
            network_in: VecDeque::with_capacity(max_samples),
            network_out: VecDeque::with_capacity(max_samples),
            blocks: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    fn add_sample(&mut self, cpu: f32, memory: u64, network: u64, blocks: u64) {
        if self.cpu.len() >= self.max_samples {
            self.cpu.pop_front();
            self.memory.pop_front();
            self.network_in.pop_front();
            self.network_out.pop_front();
            self.blocks.pop_front();
        }
        self.cpu.push_back(cpu);
        self.memory.push_back(memory);
        self.network_in.push_back(network);
        self.network_out.push_back(network);
        self.blocks.push_back(blocks);
    }
}

impl Dashboard {
    pub fn new(socket_path: Option<PathBuf>, refresh_interval: u64) -> Self {
        // Wallet data should be loaded from:
        // 1. Configuration file
        // 2. Node query via socket
        // 3. User input/import
        // Empty for now - no hardcoded data
        let wallets = Vec::new();

        Self {
            selected_tab: 0,
            selected_subtab: 0,
            selected_item: 0,
            socket_path,
            refresh_interval: Duration::from_secs(refresh_interval),
            last_update: Instant::now(),
            stats: NodeStats::default(),
            stake_pool: StakePoolInfo::empty(),
            wallets,
            api_settings: ApiSettings::default(),
            alerts: Vec::new(),
            logs: vec![
                "Node started successfully".to_string(),
                "Connected to network".to_string(),
                "Syncing blockchain...".to_string(),
            ],
            metrics_history: MetricsHistory::new(60),
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            should_quit: false,
            show_help: false,
        }
    }

    /// Run the dashboard
    pub async fn run(&mut self) -> Result<()> {
        // Run the UI loop
        self.run_ui().await?;
        Ok(())
    }

    async fn run_ui(&mut self) -> Result<()> {
        let mut terminal = setup_terminal()?;

        loop {
            terminal.draw(|f| self.ui(f))?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    match self.input_mode {
                        InputMode::Normal | InputMode::Command => {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    if self.show_help {
                                        self.show_help = false;
                                    } else if self.input_mode == InputMode::Command {
                                        self.input_mode = InputMode::Normal;
                                        self.input_buffer.clear();
                                    } else {
                                        self.should_quit = true;
                                    }
                                }
                                KeyCode::Char('?') => self.show_help = !self.show_help,
                                KeyCode::Char(':') => {
                                    self.input_mode = InputMode::Command;
                                    self.input_buffer.clear();
                                }
                                KeyCode::Tab => self.selected_tab = (self.selected_tab + 1) % 8,
                                KeyCode::BackTab => {
                                    self.selected_tab = if self.selected_tab == 0 {
                                        7
                                    } else {
                                        self.selected_tab - 1
                                    }
                                }
                                KeyCode::Char('1') => self.selected_tab = 0,
                                KeyCode::Char('2') => self.selected_tab = 1,
                                KeyCode::Char('3') => self.selected_tab = 2,
                                KeyCode::Char('4') => self.selected_tab = 3,
                                KeyCode::Char('5') => self.selected_tab = 4,
                                KeyCode::Char('6') => self.selected_tab = 5,
                                KeyCode::Char('7') => self.selected_tab = 6,
                                KeyCode::Char('8') => self.selected_tab = 7,
                                KeyCode::Up => {
                                    if self.selected_item > 0 {
                                        self.selected_item -= 1;
                                    }
                                }
                                KeyCode::Down => {
                                    let max_items = match self.selected_tab {
                                        2 => self.wallets.len().saturating_sub(1), // Wallets tab
                                        6 => self.alerts.len().saturating_sub(1),  // Alerts tab
                                        _ => 0,
                                    };
                                    if self.selected_item < max_items {
                                        self.selected_item += 1;
                                    }
                                }
                                KeyCode::Enter => {
                                    if self.input_mode == InputMode::Command {
                                        self.execute_command();
                                    } else if self.selected_tab == 6
                                        && self.selected_item < self.alerts.len()
                                    {
                                        self.alerts[self.selected_item].acknowledged = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                        InputMode::Editing => {
                            match key.code {
                                KeyCode::Enter => {
                                    self.input_mode = InputMode::Normal;
                                    // Apply the edited value
                                    self.apply_input();
                                }
                                KeyCode::Char(c) => {
                                    self.input_buffer.push(c);
                                }
                                KeyCode::Backspace => {
                                    self.input_buffer.pop();
                                }
                                KeyCode::Esc => {
                                    self.input_mode = InputMode::Normal;
                                    self.input_buffer.clear();
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }

            if self.should_quit {
                break;
            }

            if self.last_update.elapsed() >= self.refresh_interval {
                self.update_stats().await?;
                self.last_update = Instant::now();
            }
        }

        restore_terminal()?;
        Ok(())
    }

    fn execute_command(&mut self) {
        let cmd = self.input_buffer.trim();
        match cmd {
            "quit" | "q" => self.should_quit = true,
            "help" | "h" => self.show_help = true,
            "clear" => self.logs.clear(),
            _ => {
                self.logs.push(format!("Unknown command: {}", cmd));
            }
        }
        self.input_buffer.clear();
        self.input_mode = InputMode::Normal;
    }

    fn apply_input(&mut self) {
        // Apply edited values to api_settings or other editable fields
        // This will be expanded when we add edit mode for specific fields
        self.input_buffer.clear();
    }

    fn ui(&mut self, f: &mut Frame) {
        if self.show_help {
            self.render_help(f);
            return;
        }

        let size = f.area();

        // Create main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(3), // Tabs
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Status bar
            ])
            .split(size);

        // Title
        let title = Paragraph::new("🚀 Cardano Node Dashboard")
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, chunks[0]);

        // Tabs - now with 8 tabs instead of 4
        let tabs = Tabs::new(vec![
            "Overview",
            "Stake Pool",
            "Wallets",
            "Network",
            "Monitoring",
            "API Settings",
            "Alerts",
            "Logs",
        ])
        .select(self.selected_tab)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(tabs, chunks[1]);

        // Content based on selected tab
        match self.selected_tab {
            0 => self.render_overview(f, chunks[2]),
            1 => self.render_stake_pool(f, chunks[2]),
            2 => self.render_wallets(f, chunks[2]),
            3 => self.render_network(f, chunks[2]),
            4 => self.render_monitoring(f, chunks[2]),
            5 => self.render_api_settings(f, chunks[2]),
            6 => self.render_alerts(f, chunks[2]),
            7 => self.render_logs(f, chunks[2]),
            _ => {}
        }

        // Status bar - enhanced with input mode display
        let status_text = if self.input_mode == InputMode::Command {
            Line::from(vec![
                Span::styled(": ", Style::default().fg(Color::Yellow)),
                Span::raw(&self.input_buffer),
            ])
        } else if self.input_mode == InputMode::Editing {
            Line::from(vec![
                Span::styled("EDIT: ", Style::default().fg(Color::Green)),
                Span::raw(&self.input_buffer),
            ])
        } else {
            Line::from(vec![
                Span::raw("Press "),
                Span::styled(
                    "q",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" to quit | "),
                Span::styled("1-8", Style::default().fg(Color::Yellow)),
                Span::raw(" to switch tabs | "),
                Span::styled("?", Style::default().fg(Color::Cyan)),
                Span::raw(" for help"),
            ])
        };

        let status = Paragraph::new(status_text)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(status, chunks[3]);
    }

    fn render_help(&self, f: &mut Frame) {
        let help_text = vec![
            "╔════════════════════════════════════════════════════════╗",
            "║        Cardano Node Dashboard - Help                  ║",
            "╚════════════════════════════════════════════════════════╝",
            "",
            "Navigation:",
            "  [1-8]         - Switch to specific tab",
            "  [Tab]         - Next tab",
            "  [Shift+Tab]   - Previous tab",
            "  [↑/↓]         - Navigate items in lists",
            "  [Enter]       - Select/Acknowledge item",
            "",
            "Commands:",
            "  [:]           - Enter command mode",
            "  [:quit or :q] - Quit dashboard",
            "  [:help or :h] - Show this help",
            "  [:clear]      - Clear logs",
            "",
            "Tabs:",
            "  1. Overview      - Node status and blockchain info",
            "  2. Stake Pool    - Pool operations and statistics",
            "  3. Wallets       - Wallet management and balances",
            "  4. Network       - Peer connections and topology",
            "  5. Monitoring    - Performance metrics and graphs",
            "  6. API Settings  - Configure REST/WebSocket/Prometheus",
            "  7. Alerts        - System alerts and notifications",
            "  8. Logs          - Node activity logs",
            "",
            "Press [q] or [Esc] to close this help",
        ];

        let help_paragraph = Paragraph::new(help_text.join("\n"))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Help")
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White));

        let area = centered_rect(70, 80, f.area());
        f.render_widget(Clear, area);
        f.render_widget(help_paragraph, area);
    }

    fn render_overview(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Sync gauge
                Constraint::Length(3), // Uptime
                Constraint::Min(0),    // Stats table
            ])
            .split(area);

        // Sync progress
        let gauge = Gauge::default()
            .block(
                Block::default()
                    .title("Sync Progress")
                    .borders(Borders::ALL),
            )
            .gauge_style(Style::default().fg(Color::Green))
            .percent((self.stats.sync_progress * 100.0) as u16)
            .label(format!("{:.2}%", self.stats.sync_progress * 100.0));
        f.render_widget(gauge, chunks[0]);

        // Uptime
        let uptime_secs = self.stats.uptime.as_secs();
        let hours = uptime_secs / 3600;
        let minutes = (uptime_secs % 3600) / 60;
        let seconds = uptime_secs % 60;
        let uptime = Paragraph::new(format!(
            "Uptime: {:02}:{:02}:{:02}",
            hours, minutes, seconds
        ))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
        f.render_widget(uptime, chunks[1]);

        // Stats table
        let stats_rows = vec![
            Row::new(vec![
                Cell::from("Chain Tip"),
                Cell::from(self.stats.chain_tip.to_string()),
            ]),
            Row::new(vec![
                Cell::from("Total Blocks"),
                Cell::from(self.stats.total_blocks.to_string()),
            ]),
            Row::new(vec![
                Cell::from("Peers Connected"),
                Cell::from(self.stats.peer_count.to_string()),
            ]),
            Row::new(vec![
                Cell::from("Transactions Processed"),
                Cell::from(self.stats.tx_processed.to_string()),
            ]),
            Row::new(vec![
                Cell::from("Mempool Size"),
                Cell::from(self.stats.mempool_size.to_string()),
            ]),
            Row::new(vec![
                Cell::from("CPU Usage"),
                Cell::from(format!("{:.1}%", self.stats.cpu_usage)),
            ]),
            Row::new(vec![
                Cell::from("Memory Usage"),
                Cell::from(format!("{} MB", self.stats.memory_usage / 1024 / 1024)),
            ]),
            Row::new(vec![
                Cell::from("Disk Usage"),
                Cell::from(format!("{} GB", self.stats.disk_usage / 1024 / 1024 / 1024)),
            ]),
        ];

        let stats_table = Table::new(
            stats_rows,
            [Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .block(
            Block::default()
                .title("Node Statistics")
                .borders(Borders::ALL),
        )
        .header(
            Row::new(vec!["Metric", "Value"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        );
        f.render_widget(stats_table, chunks[2]);
    }

    fn render_network(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Min(0)])
            .split(area);

        // Network stats
        let network_info = [
            format!("Connected Peers: {}", self.stats.peer_count),
            format!("Network In: {} MB", self.stats.network_in / 1024 / 1024),
            format!("Network Out: {} MB", self.stats.network_out / 1024 / 1024),
            format!(
                "Bandwidth: ↓ {:.2} MB/s ↑ {:.2} MB/s",
                self.stats.network_in as f64 / 1024.0 / 1024.0 / self.stats.uptime.as_secs() as f64,
                self.stats.network_out as f64
                    / 1024.0
                    / 1024.0
                    / self.stats.uptime.as_secs() as f64
            ),
        ];
        let network_para = Paragraph::new(network_info.join("\n")).block(
            Block::default()
                .title("Network Statistics")
                .borders(Borders::ALL),
        );
        f.render_widget(network_para, chunks[0]);

        // Peer list (mock data)
        let peers: Vec<ListItem> = (0..self.stats.peer_count.min(20))
            .map(|i| {
                ListItem::new(format!(
                    "Peer {}: 192.168.1.{} ({}ms)",
                    i + 1,
                    10 + i,
                    (i + 1) * 5
                ))
            })
            .collect();
        let peers_list = List::new(peers).block(
            Block::default()
                .title("Connected Peers")
                .borders(Borders::ALL),
        );
        f.render_widget(peers_list, chunks[1]);
    }

    fn render_logs(&self, f: &mut Frame, area: Rect) {
        let logs: Vec<ListItem> = self
            .logs
            .iter()
            .rev()
            .take(area.height as usize - 2)
            .map(|log| ListItem::new(log.clone()))
            .collect();

        let logs_list = List::new(logs)
            .block(Block::default().title("Node Logs").borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray));
        f.render_widget(logs_list, area);
    }

    fn render_stake_pool(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8), // Pool Info
                Constraint::Min(0),    // Performance Stats
            ])
            .split(area);

        // Pool Info
        let pool_status_color = match self.stake_pool.status {
            PoolStatus::Active => Color::Green,
            PoolStatus::Retiring => Color::Yellow,
            PoolStatus::Retired => Color::Red,
            PoolStatus::Inactive => Color::Gray,
        };

        let pool_info = [
            format!("Pool ID: {}", self.stake_pool.pool_id),
            format!(
                "Ticker: {} | Name: {}",
                self.stake_pool.ticker, self.stake_pool.name
            ),
            format!("Status: {:?}", self.stake_pool.status),
            format!(
                "Pledge: {} ₳ | Margin: {:.2}% | Fixed Cost: {} ₳",
                self.stake_pool.pledge / 1_000_000,
                self.stake_pool.margin * 100.0,
                self.stake_pool.fixed_cost / 1_000_000
            ),
            format!(
                "Active Stake: {} ₳ ({:.2}% saturation)",
                self.stake_pool.active_stake / 1_000_000,
                self.stake_pool.saturation * 100.0
            ),
            format!(
                "Delegators: {} | Rank: #{}",
                self.stake_pool.delegators, self.stake_pool.rank
            ),
        ];

        let info_widget = Paragraph::new(pool_info.join("\n"))
            .block(
                Block::default()
                    .title("Stake Pool Information")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(pool_status_color)),
            )
            .style(Style::default().fg(Color::White));
        f.render_widget(info_widget, chunks[0]);

        // Performance Stats
        let perf_data = [
            vec![
                "Metric".to_string(),
                "Current Epoch".to_string(),
                "Lifetime".to_string(),
            ],
            vec![
                "Blocks Minted".to_string(),
                format!(
                    "{} / {} expected",
                    self.stake_pool.blocks_minted, self.stake_pool.blocks_expected
                ),
                self.stake_pool.lifetime_blocks.to_string(),
            ],
            vec![
                "ROA".to_string(),
                format!("{:.2}%", self.stake_pool.roa * 100.0),
                "N/A".to_string(),
            ],
            vec![
                "Performance".to_string(),
                if self.stake_pool.blocks_expected > 0 {
                    format!(
                        "{:.1}%",
                        (self.stake_pool.blocks_minted as f64
                            / self.stake_pool.blocks_expected as f64)
                            * 100.0
                    )
                } else {
                    "N/A".to_string()
                },
                "N/A".to_string(),
            ],
        ];

        let table = Table::new(
            perf_data
                .iter()
                .map(|row| Row::new(row.iter().map(|c| Cell::from(c.as_str())))),
            [
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ],
        )
        .block(
            Block::default()
                .title("Performance Metrics")
                .borders(Borders::ALL),
        )
        .header(
            Row::new(vec!["Metric", "Current Epoch", "Lifetime"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .style(Style::default().fg(Color::White));

        f.render_widget(table, chunks[1]);
    }

    fn render_wallets(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40), // Wallet List
                Constraint::Percentage(60), // Wallet Details
            ])
            .split(area);

        // Wallet List
        let wallet_items: Vec<ListItem> = self
            .wallets
            .iter()
            .enumerate()
            .map(|(i, wallet)| {
                let content = format!("{} - {} ₳", wallet.name, wallet.balance / 1_000_000);
                let style = if i == self.selected_item {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(content).style(style)
            })
            .collect();

        let wallet_list = List::new(wallet_items)
            .block(Block::default().title("Wallets").borders(Borders::ALL))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );
        f.render_widget(wallet_list, chunks[0]);

        // Wallet Details
        if let Some(wallet) = self.wallets.get(self.selected_item) {
            let details = vec![
                format!("Name: {}", wallet.name),
                format!("Address: {}", wallet.address),
                format!("Balance: {} ₳", wallet.balance / 1_000_000),
                format!("Staked: {} ₳", wallet.staked / 1_000_000),
                format!("Rewards: {} ₳", wallet.rewards / 1_000_000),
                format!(
                    "Delegated to: {}",
                    wallet
                        .delegated_pool
                        .as_ref()
                        .unwrap_or(&"None".to_string())
                ),
                format!("UTxO Count: {}", wallet.utxo_count),
                format!("Transactions: {}", wallet.tx_count),
                "".to_string(),
                "Actions:".to_string(),
                "  [d] Delegate stake".to_string(),
                "  [w] Withdraw rewards".to_string(),
                "  [s] Send ADA".to_string(),
            ];

            let details_widget = Paragraph::new(details.join("\n"))
                .block(
                    Block::default()
                        .title("Wallet Details")
                        .borders(Borders::ALL),
                )
                .style(Style::default().fg(Color::White))
                .wrap(Wrap { trim: true });
            f.render_widget(details_widget, chunks[1]);
        }
    }

    fn render_monitoring(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50), // Graphs
                Constraint::Percentage(50), // Recent Metrics
            ])
            .split(area);

        let graph_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);

        // CPU Usage Graph
        let cpu_data: Vec<(f64, f64)> = self
            .metrics_history
            .cpu
            .iter()
            .enumerate()
            .map(|(i, &val)| (i as f64, val as f64))
            .collect();

        let cpu_dataset = vec![Dataset::default()
            .name("CPU %")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Cyan))
            .data(&cpu_data)];

        let cpu_chart = Chart::new(cpu_dataset)
            .block(Block::default().title("CPU Usage").borders(Borders::ALL))
            .x_axis(
                Axis::default()
                    .bounds([0.0, self.metrics_history.max_samples as f64])
                    .labels(vec![
                        Span::raw("0"),
                        Span::raw(format!("{}", self.metrics_history.max_samples / 2)),
                        Span::raw(format!("{}", self.metrics_history.max_samples)),
                    ]),
            )
            .y_axis(Axis::default().bounds([0.0, 100.0]).labels(vec![
                Span::raw("0%"),
                Span::raw("50%"),
                Span::raw("100%"),
            ]));
        f.render_widget(cpu_chart, graph_chunks[0]);

        // Memory Usage Graph
        let mem_data: Vec<(f64, f64)> = self
            .metrics_history
            .memory
            .iter()
            .enumerate()
            .map(|(i, &val)| (i as f64, (val / 1_000_000_000) as f64))
            .collect();

        let mem_dataset = vec![Dataset::default()
            .name("Memory GB")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Green))
            .data(&mem_data)];

        let mem_chart = Chart::new(mem_dataset)
            .block(
                Block::default()
                    .title("Memory Usage (GB)")
                    .borders(Borders::ALL),
            )
            .x_axis(
                Axis::default()
                    .bounds([0.0, self.metrics_history.max_samples as f64])
                    .labels(vec![
                        Span::raw("0"),
                        Span::raw(format!("{}", self.metrics_history.max_samples / 2)),
                        Span::raw(format!("{}", self.metrics_history.max_samples)),
                    ]),
            )
            .y_axis(Axis::default().bounds([0.0, 8.0]).labels(vec![
                Span::raw("0GB"),
                Span::raw("4GB"),
                Span::raw("8GB"),
            ]));
        f.render_widget(mem_chart, graph_chunks[1]);

        // Recent Metrics Table
        let metrics_data = [
            vec![
                "Metric".to_string(),
                "Current".to_string(),
                "Average".to_string(),
            ],
            vec![
                "CPU".to_string(),
                format!("{:.1}%", self.stats.cpu_usage),
                format!(
                    "{:.1}%",
                    self.metrics_history.cpu.iter().sum::<f32>()
                        / self.metrics_history.cpu.len() as f32
                ),
            ],
            vec![
                "Memory".to_string(),
                format!("{:.2} GB", self.stats.memory_usage as f64 / 1_000_000_000.0),
                format!(
                    "{:.2} GB",
                    self.metrics_history.memory.iter().sum::<u64>() as f64
                        / self.metrics_history.memory.len() as f64
                        / 1_000_000_000.0
                ),
            ],
            vec![
                "Network In".to_string(),
                format!("{:.2} MB/s", self.stats.network_in as f64 / 1_000_000.0),
                "N/A".to_string(),
            ],
            vec![
                "Network Out".to_string(),
                format!("{:.2} MB/s", self.stats.network_out as f64 / 1_000_000.0),
                "N/A".to_string(),
            ],
        ];

        let metrics_table = Table::new(
            metrics_data
                .iter()
                .map(|row| Row::new(row.iter().map(|c| Cell::from(c.as_str())))),
            [
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ],
        )
        .block(
            Block::default()
                .title("System Metrics")
                .borders(Borders::ALL),
        )
        .header(
            Row::new(vec!["Metric", "Current", "Average"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .style(Style::default().fg(Color::White));

        f.render_widget(metrics_table, chunks[1]);
    }

    fn render_api_settings(&self, f: &mut Frame, area: Rect) {
        let settings_text = vec![
            "API Configuration".to_string(),
            "".to_string(),
            format!(
                "REST API:       {} (Port: {})",
                if self.api_settings.rest_enabled {
                    "✓ Enabled"
                } else {
                    "✗ Disabled"
                },
                self.api_settings.rest_port
            ),
            format!(
                "WebSocket:      {} (Port: {})",
                if self.api_settings.websocket_enabled {
                    "✓ Enabled"
                } else {
                    "✗ Disabled"
                },
                self.api_settings.websocket_port
            ),
            format!(
                "Prometheus:     {} (Port: {})",
                if self.api_settings.prometheus_enabled {
                    "✓ Enabled"
                } else {
                    "✗ Disabled"
                },
                self.api_settings.prometheus_port
            ),
            format!(
                "EKG:            {} (Port: {})",
                if self.api_settings.ekg_enabled {
                    "✓ Enabled"
                } else {
                    "✗ Disabled"
                },
                self.api_settings.ekg_port
            ),
            "".to_string(),
            format!(
                "Authentication: {}",
                if self.api_settings.auth_required {
                    "✓ Required"
                } else {
                    "✗ Not Required"
                }
            ),
            format!("Rate Limiting:  {} req/min", self.api_settings.rate_limit),
            "".to_string(),
            "Configuration Options:".to_string(),
            "  [r] Toggle REST API".to_string(),
            "  [w] Toggle WebSocket".to_string(),
            "  [p] Toggle Prometheus".to_string(),
            "  [e] Toggle EKG".to_string(),
            "  [a] Toggle Authentication".to_string(),
            "  [l] Set Rate Limit".to_string(),
        ];

        let settings_widget = Paragraph::new(settings_text.join("\n"))
            .block(Block::default().title("API Settings").borders(Borders::ALL))
            .style(Style::default().fg(Color::White));
        f.render_widget(settings_widget, area);
    }

    fn render_alerts(&self, f: &mut Frame, area: Rect) {
        let alert_items: Vec<ListItem> = self
            .alerts
            .iter()
            .enumerate()
            .map(|(i, alert)| {
                let color = match alert.level {
                    AlertLevel::Info => Color::Cyan,
                    AlertLevel::Warning => Color::Yellow,
                    AlertLevel::Error => Color::LightRed,
                    AlertLevel::Critical => Color::Red,
                };

                let prefix = if alert.acknowledged { "✓" } else { "•" };
                let content = format!(
                    "{} [{:?}] {}: {}",
                    prefix,
                    alert.level,
                    alert.timestamp.format("%H:%M:%S"),
                    alert.message
                );

                let style = if i == self.selected_item {
                    Style::default()
                        .fg(color)
                        .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                } else if alert.acknowledged {
                    Style::default().fg(Color::Gray)
                } else {
                    Style::default().fg(color)
                };

                ListItem::new(content).style(style)
            })
            .collect();

        let alerts_list = List::new(alert_items)
            .block(
                Block::default()
                    .title("System Alerts (Press Enter to acknowledge)")
                    .borders(Borders::ALL),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        f.render_widget(alerts_list, area);
    }

    async fn update_stats(&mut self) -> Result<()> {
        // NOTE: This currently uses mock/simulated data for demonstration
        // In production, this would connect to the running node via:
        // - Unix domain socket (default: /tmp/cardano-node.socket)
        // - REST API endpoint
        // - Direct IPC channel
        // The node would provide real-time metrics from the ledger state

        // ============================================================================
        // PRODUCTION NOTE: This section uses placeholder incrementing for demonstration
        // In production, ALL data should come from:
        // 1. Unix socket queries to running node (chain-tip, blocks, sync)
        // 2. Network layer queries (peer count, network traffic)
        // 3. System metrics (CPU via sysinfo crate, memory, disk)
        // 4. Mempool queries via node socket
        //
        // NO random data or hardcoded values should be used in production!
        // ============================================================================

        // Minimal incrementing for demonstration (not real data)
        self.stats.chain_tip += 1;
        self.stats.total_blocks = self.stats.chain_tip + 100;
        self.stats.sync_progress =
            (self.stats.chain_tip as f64 / self.stats.total_blocks as f64).min(1.0);
        self.stats.uptime += self.refresh_interval;

        // These should come from actual queries:
        // self.stats.peer_count = node_client.get_peer_count().await?;
        // self.stats.tx_processed = node_client.get_tx_count().await?;
        // self.stats.mempool_size = node_client.get_mempool_size().await?;
        // self.stats.cpu_usage = system.cpu_usage();
        // self.stats.memory_usage = system.memory_usage();
        // self.stats.disk_usage = system.disk_usage();

        // Placeholder - no fake logging in production
        if self.stats.chain_tip % 10 == 0 {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let hours = (now / 3600) % 24;
            let minutes = (now / 60) % 60;
            let seconds = now % 60;
            self.logs.push(format!(
                "[{:02}:{:02}:{:02}] Awaiting node connection",
                hours, minutes, seconds
            ));
            if self.logs.len() > 100 {
                self.logs.remove(0);
            }
        }

        // Update metrics history
        self.metrics_history.add_sample(
            self.stats.cpu_usage,
            self.stats.memory_usage,
            self.stats.network_in,
            self.stats.chain_tip,
        );

        Ok(())
    }
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Setup terminal for TUI
fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore terminal to normal mode
fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

// Mock rand for demo purposes
#[allow(dead_code)]
mod rand {
    #[allow(dead_code)]
    pub fn random<T>() -> T
    where
        T: From<u8>,
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        T::from((nanos % 256) as u8)
    }
}
