use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Negotiating,
    Playing,
    Teardown,
}

#[derive(Debug, Clone)]
pub struct StreamSession {
    pub session_id: String,
    pub state: SessionState,
}

impl StreamSession {
    pub fn transition(&mut self, next: SessionState) {
        info!(session_id = %self.session_id, from = ?self.state, to = ?next, "gamestream session transition");
        self.state = next;
    }
}
