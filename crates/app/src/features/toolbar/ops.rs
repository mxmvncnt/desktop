//! Fetch / pull / push on [`RepoModel`].

use gpui::Context;

use crate::repo::{RemoteOp, RepoModel};

impl RepoModel {
    pub fn remote(&mut self, op: RemoteOp, cx: &mut Context<Self>) {
        if self.remote_op.is_some() {
            return;
        }
        self.remote_op = Some(op);
        cx.notify();
        let (branch, has_upstream) = match &self.status {
            Some(s) => (s.branch.clone().unwrap_or_default(), s.upstream.is_some()),
            None => (String::new(), false),
        };
        self.run(
            move |r| match op {
                RemoteOp::Fetch => r.fetch(),
                RemoteOp::Pull => r.pull(),
                RemoteOp::Push => r.push(&branch, has_upstream),
            },
            move |this, _, _| this.notice = Some(done_message(op).into()),
            cx,
        );
    }
}

fn done_message(op: RemoteOp) -> &'static str {
    match op {
        RemoteOp::Fetch => "Fetched from remote",
        RemoteOp::Pull => "Pulled from remote",
        RemoteOp::Push => "Pushed to remote",
    }
}
