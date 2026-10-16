//! Account-local notification decisions over already verified sync results.
//! This module performs no OS presentation, network request or plaintext search.
use crate::client::SyncResult;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};

#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Route {
    pub server: String,
    pub channel: String,
}

/// Kept in the existing encrypted, account-isolated UI preference document.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub enabled: bool,
    pub previews: bool,
    pub do_not_disturb: bool,
    pub muted_servers: BTreeSet<String>,
    pub muted_routes: BTreeSet<Route>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: true,
            previews: false,
            do_not_disturb: false,
            muted_servers: BTreeSet::new(),
            muted_routes: BTreeSet::new(),
        }
    }
}

#[derive(Debug)]
pub struct Context<'a> {
    pub account: &'a str,
    pub unlocked: bool,
    pub visible_route: Option<Route>,
    /// OS presenter availability and OS DND allow this delivery now.
    pub platform_allowed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Approved {
    pub id: String,
    pub account: String,
    pub route: Route,
    pub title: String,
    pub body: String,
}

/// Reset on account lock/switch. First successful sync per server is a baseline,
/// never a source of OS alerts. `SQLite` cursors remain the durable replay boundary.
#[derive(Debug, Default)]
pub struct Classifier {
    baselines: BTreeSet<(String, String)>,
    seen: BTreeSet<(String, Route, String)>,
    order: VecDeque<(String, Route, String)>,
}
impl Classifier {
    /// Classify verified new deliveries, coalescing each route to one alert.
    /// Suppressed deliveries are observed too, so changing settings cannot turn
    /// an old or previously visible message into a fresh notification.
    /// # Errors
    /// Rejects inconsistent local delivery metadata rather than misrouting it.
    pub fn classify(
        &mut self,
        server: &str,
        result: &SyncResult,
        context: &Context<'_>,
        preferences: &Preferences,
    ) -> Result<Vec<Approved>> {
        ensure!(
            result.message_routes.len() == result.messages.len(),
            "Synchronization notification route metadata is inconsistent"
        );
        let established = !self
            .baselines
            .insert((context.account.into(), server.into()));
        let mut routes = BTreeSet::new();
        let mut alerts = Vec::new();
        for (message, channel) in result.messages.iter().zip(&result.message_routes) {
            let route = Route {
                server: server.into(),
                channel: channel.clone(),
            };
            let key = (
                context.account.into(),
                route.clone(),
                message.message_id.clone(),
            );
            if !self.seen.insert(key.clone()) {
                continue;
            }
            self.order.push_back(key);
            if self.order.len() > 8192
                && let Some(old) = self.order.pop_front()
            {
                self.seen.remove(&old);
            }
            if !established
                || !context.unlocked
                || !context.platform_allowed
                || !preferences.enabled
                || preferences.do_not_disturb
                || message.sender_account_id == context.account
                || context.visible_route.as_ref() == Some(&route)
                || preferences.muted_servers.contains(server)
                || preferences.muted_routes.contains(&route)
                || !routes.insert(route.clone())
                || alerts.len() >= 32
            {
                continue;
            }
            alerts.push(Approved {
                id: uuid::Uuid::now_v7().to_string(),
                account: context.account.into(),
                route,
                title: "Cords".into(),
                body: if preferences.previews {
                    message.body.chars().take(256).collect()
                } else {
                    "New message".into()
                },
            });
        }
        Ok(alerts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cords_protocol::messaging::Message;
    fn result(id: &str, sender: &str) -> SyncResult {
        SyncResult {
            fetched: 1,
            message_routes: vec!["channel".into()],
            messages: vec![Message {
                schema_version: 1,
                message_id: id.into(),
                sender_account_id: sender.into(),
                sender_device_id: "device".into(),
                client_timestamp: 1,
                event_kind: "message.create".into(),
                body: "private content".into(),
            }],
        }
    }
    #[test]
    fn suppresses_history_duplicates_own_visible_muted_locked_and_dnd() -> Result<()> {
        let mut classifier = Classifier::default();
        let mut context = Context {
            account: "self",
            unlocked: true,
            visible_route: None,
            platform_allowed: true,
        };
        let mut preferences = Preferences {
            enabled: true,
            ..Preferences::default()
        };
        assert!(
            classifier
                .classify(
                    "server",
                    &result("history", "other"),
                    &context,
                    &preferences
                )?
                .is_empty()
        );
        let incoming = result("new", "other");
        let alerts = classifier.classify("server", &incoming, &context, &preferences)?;
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].body, "New message");
        assert!(!alerts[0].body.contains("private content"));
        assert!(
            classifier
                .classify("server", &incoming, &context, &preferences)?
                .is_empty()
        );
        assert!(
            classifier
                .classify("server", &result("own", "self"), &context, &preferences)?
                .is_empty()
        );
        context.visible_route = Some(Route {
            server: "server".into(),
            channel: "channel".into(),
        });
        assert!(
            classifier
                .classify(
                    "server",
                    &result("visible", "other"),
                    &context,
                    &preferences
                )?
                .is_empty()
        );
        context.visible_route = None;
        preferences.muted_servers.insert("server".into());
        assert!(
            classifier
                .classify("server", &result("muted", "other"), &context, &preferences)?
                .is_empty()
        );
        preferences.muted_servers.clear();
        preferences.do_not_disturb = true;
        assert!(
            classifier
                .classify("server", &result("dnd", "other"), &context, &preferences)?
                .is_empty()
        );
        preferences.do_not_disturb = false;
        context.unlocked = false;
        assert!(
            classifier
                .classify("server", &result("locked", "other"), &context, &preferences)?
                .is_empty()
        );
        context.unlocked = true;
        context.platform_allowed = false;
        assert!(
            classifier
                .classify("server", &result("os-dnd", "other"), &context, &preferences)?
                .is_empty()
        );
        context.platform_allowed = true;
        preferences.previews = true;
        let preview = classifier.classify(
            "server",
            &result("preview", "other"),
            &context,
            &preferences,
        )?;
        assert_eq!(preview[0].body, "private content");
        let mut invalid = result("invalid", "other");
        invalid.message_routes.clear();
        assert!(
            classifier
                .classify("server", &invalid, &context, &preferences)
                .is_err()
        );
        Ok(())
    }
}
