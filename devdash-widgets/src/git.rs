// devdash-widgets/src/git.rs
use devdash_core::{
    EventBus, EventResult, Widget,
    event::{Event, Subscription},
};
use git2::{BranchType, Repository, StatusOptions};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    prelude::Widget as RatatuiWidget,
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::path::PathBuf;
use std::time::Duration;

use crate::common::focus_color;

/// Git repository status information
#[derive(Debug, Clone)]
pub struct GitStatus {
    pub branch: String,
    pub remote_branch: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub staged: usize,
    pub unstaged: usize,
    pub untracked: usize,
    pub last_commits: Vec<CommitInfo>,
}

/// Git commit information for display
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: String,    // Short hash (7 chars)
    pub message: String, // First line only
    pub author: String,
}

/// Git repository monitoring widget with status and commit history
///
/// Displays current git repository status including branch information,
/// file counts, and recent commit history. Shows "No repository" when
/// not in a git repository.
///
/// # Keyboard Shortcuts
/// - `g` - Open current directory in file manager
/// - `r` - Force refresh git status
///
/// # Event Publishing
/// - Publishes `system.git.status` events with current git status
pub struct GitWidget {
    repo_path: PathBuf,        // Current directory
    status: Option<GitStatus>, // None if not in repo
    poll_interval: Duration,
    time_since_poll: Duration,
    event_bus: EventBus,
    _subscription: Option<Subscription>,
}

impl GitWidget {
    /// Create a new GitWidget with specified poll interval
    ///
    /// # Arguments
    /// * `event_bus` - Event bus for publishing git status
    /// * `poll_interval` - How often to refresh git status
    pub fn new(event_bus: EventBus, poll_interval: Duration) -> Self {
        Self {
            repo_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            status: None,
            poll_interval,
            time_since_poll: Duration::ZERO,
            event_bus,
            _subscription: None,
        }
    }

    /// Poll git repository for current status
    fn poll_git_status(&mut self) {
        match Repository::open(&self.repo_path) {
            Ok(repo) => {
                self.status = Some(GitStatus::from_repo(&repo));

                // Publish git status event
                if let Some(ref status) = self.status {
                    self.event_bus.publish(Event::new(
                        "system.git.status",
                        format!(
                            "branch={}, staged={}, unstaged={}, untracked={}, ahead={}, behind={}",
                            status.branch,
                            status.staged,
                            status.unstaged,
                            status.untracked,
                            status.ahead,
                            status.behind
                        ),
                    ));
                }
            }
            Err(_) => {
                self.status = None;
            }
        }
    }

    /// Open current directory in file manager
    fn open_file_manager(&self) {
        let path = self.repo_path.to_string_lossy().to_string();

        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/c", "start", &path])
                .spawn()
                .ok();
        }

        #[cfg(not(target_os = "windows"))]
        {
            std::process::Command::new("open").arg(&path).spawn().ok();
        }
    }
}

impl GitStatus {
    /// Create GitStatus from a git repository
    fn from_repo(repo: &Repository) -> Self {
        // Get current branch
        let branch = repo
            .head()
            .ok()
            .and_then(|head| head.shorthand().map(|s| s.to_string()))
            .unwrap_or_else(|| "detached".to_string());

        // Get remote branch info
        let (remote_branch, ahead, behind) = repo
            .head()
            .ok()
            .and_then(|head| head.resolve().ok())
            .and_then(|head| {
                let branch_name = head.shorthand()?;
                let branch = repo.find_branch(branch_name, BranchType::Local).ok()?;
                let upstream = branch.upstream().ok()?;
                let upstream_name = upstream.name().ok()??;

                let local_oid = head.target()?;
                let upstream_oid = upstream.get().target()?;

                let (ahead, behind) = repo.graph_ahead_behind(local_oid, upstream_oid).ok()?;

                Some((Some(upstream_name.to_string()), ahead, behind))
            })
            .unwrap_or((None, 0, 0));

        // Get file status counts
        let (staged, unstaged, untracked) = repo
            .statuses(Some(StatusOptions::default().include_untracked(true)))
            .map(|statuses| {
                let mut staged = 0;
                let mut unstaged = 0;
                let mut untracked = 0;

                for entry in statuses.iter() {
                    let status = entry.status();
                    if status.is_index_new()
                        || status.is_index_modified()
                        || status.is_index_deleted()
                    {
                        staged += 1;
                    }
                    if status.is_wt_modified() || status.is_wt_deleted() {
                        unstaged += 1;
                    }
                    if status.is_wt_new() {
                        untracked += 1;
                    }
                }

                (staged, unstaged, untracked)
            })
            .unwrap_or((0, 0, 0));

        // Get last 5 commits
        let last_commits = repo
            .head()
            .ok()
            .and_then(|head| head.resolve().ok())
            .and_then(|head| head.target())
            .and_then(|oid| {
                let mut revwalk = repo.revwalk().ok()?;
                revwalk.push(oid).ok()?;
                revwalk.set_sorting(git2::Sort::TIME).ok()?;

                let mut commits = Vec::new();
                for commit_oid in revwalk.take(5).flatten() {
                    if let Ok(commit) = repo.find_commit(commit_oid) {
                        let hash = format!("{}", commit.id())[..7].to_string();
                        let message = commit
                            .message()
                            .unwrap_or("")
                            .lines()
                            .next()
                            .unwrap_or("")
                            .to_string();
                        let author = commit.author().name().unwrap_or("").to_string();

                        commits.push(CommitInfo {
                            hash,
                            message,
                            author,
                        });
                    }
                }
                Some(commits)
            })
            .unwrap_or_default();

        Self {
            branch,
            remote_branch,
            ahead,
            behind,
            staged,
            unstaged,
            untracked,
            last_commits,
        }
    }
}

impl Widget for GitWidget {
    fn on_mount(&mut self) {
        self.poll_git_status(); // Initial poll

        // Subscribe to git refresh events
        let (sub, _rx) = self.event_bus.subscribe("system.git.refresh");
        self._subscription = Some(sub);
    }

    fn on_update(&mut self, delta: Duration) {
        self.time_since_poll += delta;

        if self.time_since_poll >= self.poll_interval {
            self.poll_git_status();
            self.time_since_poll = Duration::ZERO;
        }
    }

    fn on_event(&mut self, event: devdash_core::Event) -> EventResult {
        use crossterm::event::KeyCode;

        if let devdash_core::Event::Key(key) = event {
            match key.code {
                KeyCode::Char('g') => {
                    self.open_file_manager();
                    return EventResult::Consumed;
                }
                KeyCode::Char('r') => {
                    // Force refresh
                    self.time_since_poll = self.poll_interval;
                    return EventResult::Consumed;
                }
                _ => {}
            }
        }

        EventResult::Ignored
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.render_focused(area, buf, true);
    }

    fn render_focused(&mut self, area: Rect, buf: &mut Buffer, focused: bool) {
        let border_color = focus_color(focused);

        // Create main block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner_area = block.inner(area);

        if inner_area.height < 3 {
            // Not enough space, just show the block
            RatatuiWidget::render(block, area, buf);
            return;
        }

        // Create content based on git status
        if let Some(ref status) = self.status {
            // Create title with branch and remote info
            let mut title = format!(" Git [{}", status.branch);
            if let Some(ref remote) = status.remote_branch {
                title.push_str(&format!(" → {}", remote));
            }
            if status.ahead > 0 {
                title.push_str(&format!(" ↑{}", status.ahead));
            }
            if status.behind > 0 {
                title.push_str(&format!(" ↓{}", status.behind));
            }
            title.push_str("] ");

            // Create content lines
            let mut lines = Vec::new();

            // Branch line
            lines.push(Line::from(vec![
                Span::styled("Branch: ", Style::default().fg(Color::Yellow)),
                Span::styled(&status.branch, Style::default().fg(Color::White)),
                if let Some(ref remote) = status.remote_branch {
                    Span::from(format!(" → {}", remote))
                } else {
                    Span::from("")
                },
            ]));

            // Status line
            lines.push(Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::Yellow)),
                if status.staged > 0 {
                    Span::styled(
                        format!("+{} ", status.staged),
                        Style::default().fg(Color::Green),
                    )
                } else {
                    Span::from("")
                },
                if status.unstaged > 0 {
                    Span::styled(
                        format!("~{} ", status.unstaged),
                        Style::default().fg(Color::Red),
                    )
                } else {
                    Span::from("")
                },
                if status.untracked > 0 {
                    Span::styled(
                        format!("?{} ", status.untracked),
                        Style::default().fg(Color::Gray),
                    )
                } else {
                    Span::from("")
                },
                if status.staged == 0 && status.unstaged == 0 && status.untracked == 0 {
                    Span::styled("clean", Style::default().fg(Color::Green))
                } else {
                    Span::from("")
                },
            ]));

            // Commits section
            if !status.last_commits.is_empty() && inner_area.height > 4 {
                lines.push(Line::from(Span::styled(
                    "Recent commits:",
                    Style::default().fg(Color::Yellow),
                )));

                for commit in &status.last_commits {
                    lines.push(Line::from(vec![
                        Span::styled(&commit.hash, Style::default().fg(Color::Cyan)),
                        Span::from(" "),
                        Span::from(&commit.message),
                    ]));
                }
            }

            // Create paragraph
            let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true });

            RatatuiWidget::render(paragraph, inner_area, buf);

            // Update block title and render
            let block = block.title(title);
            RatatuiWidget::render(block, area, buf);
        } else {
            // No repository
            let paragraph = Paragraph::new("No git repository found in current directory")
                .style(Style::default().fg(Color::Gray))
                .wrap(Wrap { trim: true });

            RatatuiWidget::render(paragraph, inner_area, buf);

            // Update block title and render
            let block = block.title(" Git [No repository] ");
            RatatuiWidget::render(block, area, buf);
        }
    }

    fn needs_update(&self) -> bool {
        true // Always poll for updates
    }
}
