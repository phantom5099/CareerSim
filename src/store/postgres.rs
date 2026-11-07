//! PostgreSQL 存储实现。

use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::EVENT_VERSION;
use super::SessionStore;
use super::StoreError;
use super::fold_session;
use super::max_turn_id;
use super::validate_batch;
use crate::agent::Session;
use crate::agent::SessionEvent;
use crate::agent::SessionEventKind;
use crate::agent::SessionId;
use crate::agent::SessionSummary;
use crate::agent::Stage;
use crate::agent::TurnId;

const UUID_RETRIES: usize = 3;

pub struct PostgresSessionStore {
    pool: PgPool,
}

impl PostgresSessionStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn insert_session(
        &self,
        id: SessionId,
        stage: Stage,
        at: DateTime<Utc>,
        kind: serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO sessions (id, stage, created_at, updated_at, last_seq, last_turn_id) \
             VALUES ($1, $2, $3, $3, 0, 0)",
        )
        .bind(id.as_uuid())
        .bind(stage.as_str())
        .bind(at)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO session_events (session_id, seq, v, at, kind) \
             VALUES ($1, 0, $2, $3, $4)",
        )
        .bind(id.as_uuid())
        .bind(EVENT_VERSION as i32)
        .bind(at)
        .bind(kind)
        .execute(&mut *tx)
        .await?;

        tx.commit().await
    }
}

#[async_trait]
impl SessionStore for PostgresSessionStore {
    async fn create(&self, stage: Stage) -> Result<Session, StoreError> {
        for _ in 0..UUID_RETRIES {
            let id = SessionId::new();
            let at = Utc::now();
            let kind = serde_json::to_value(SessionEventKind::SessionMeta {
                session_id: id,
                stage,
            })
            .map_err(|err| StoreError::Backend(format!("encode session_meta: {err}")))?;

            match self.insert_session(id, stage, at, kind).await {
                Ok(()) => {
                    return Ok(Session {
                        id,
                        stage,
                        events: vec![SessionEvent {
                            v: EVENT_VERSION,
                            seq: 0,
                            at,
                            kind: SessionEventKind::SessionMeta {
                                session_id: id,
                                stage,
                            },
                        }],
                        created_at: at,
                        updated_at: at,
                        last_seq: 0,
                        last_turn_id: TurnId(0),
                    });
                }
                Err(err) if is_unique_violation(&err) => continue,
                Err(err) => return Err(backend("create", err)),
            }
        }

        Err(StoreError::Backend(
            "could not allocate a unique session id".to_owned(),
        ))
    }

    async fn load(&self, id: SessionId) -> Result<Session, StoreError> {
        let mut tx = self
            .pool
            .begin_with("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .await
            .map_err(|err| backend("load", err))?;

        let row = sqlx::query(
            "SELECT id, stage, created_at, updated_at, last_seq, last_turn_id \
             FROM sessions WHERE id = $1",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| backend("load", err))?;

        let Some(row) = row else {
            return Err(StoreError::NotFound);
        };

        let row_id: Uuid = row.try_get("id").map_err(|err| backend("load", err))?;
        let stage_text: String = row.try_get("stage").map_err(|err| backend("load", err))?;
        let created_at: DateTime<Utc> = row
            .try_get("created_at")
            .map_err(|err| backend("load", err))?;
        let updated_at: DateTime<Utc> = row
            .try_get("updated_at")
            .map_err(|err| backend("load", err))?;
        let last_seq: i64 = row
            .try_get("last_seq")
            .map_err(|err| backend("load", err))?;
        let last_turn_id: i64 = row
            .try_get("last_turn_id")
            .map_err(|err| backend("load", err))?;

        if row_id != id.as_uuid() {
            return Err(StoreError::Corrupt(
                "session row id does not match the requested id".to_owned(),
            ));
        }
        let stage =
            Stage::from_str(&stage_text).map_err(|err| StoreError::Corrupt(err.to_string()))?;
        let last_seq = to_u64(last_seq, "last_seq")?;
        let last_turn_id = to_u64(last_turn_id, "last_turn_id")?;

        let rows = sqlx::query(
            "SELECT seq, v, at, kind FROM session_events \
             WHERE session_id = $1 ORDER BY seq ASC",
        )
        .bind(id.as_uuid())
        .fetch_all(&mut *tx)
        .await
        .map_err(|err| backend("load", err))?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let seq: i64 = row.try_get("seq").map_err(|err| backend("load", err))?;
            let v: i32 = row.try_get("v").map_err(|err| backend("load", err))?;
            let at: DateTime<Utc> = row.try_get("at").map_err(|err| backend("load", err))?;
            let kind: serde_json::Value =
                row.try_get("kind").map_err(|err| backend("load", err))?;

            let seq = to_u64(seq, "seq")?;
            let v = u32::try_from(v)
                .map_err(|_| StoreError::Corrupt(format!("negative event version at seq {seq}")))?;
            let kind = serde_json::from_value(kind).map_err(|err| {
                StoreError::Corrupt(format!("invalid event payload at seq {seq}: {err}"))
            })?;

            events.push(SessionEvent { v, seq, at, kind });
        }

        tx.commit().await.map_err(|err| backend("load", err))?;

        fold_session(
            id,
            stage,
            created_at,
            updated_at,
            last_seq,
            TurnId(last_turn_id),
            events,
        )
    }

    async fn list(&self, limit: usize) -> Result<Vec<SessionSummary>, StoreError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = sqlx::query(
            "SELECT id, stage, created_at, updated_at, last_seq, last_turn_id \
             FROM sessions ORDER BY updated_at DESC, id DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| backend("list", err))?;

        let mut summaries = Vec::with_capacity(rows.len());
        for row in rows {
            let id: Uuid = row.try_get("id").map_err(|err| backend("list", err))?;
            let stage_text: String = row.try_get("stage").map_err(|err| backend("list", err))?;
            let stage =
                Stage::from_str(&stage_text).map_err(|err| StoreError::Corrupt(err.to_string()))?;
            let last_seq: i64 = row
                .try_get("last_seq")
                .map_err(|err| backend("list", err))?;
            let last_turn_id: i64 = row
                .try_get("last_turn_id")
                .map_err(|err| backend("list", err))?;

            summaries.push(SessionSummary {
                id: SessionId::from_uuid(id),
                stage,
                created_at: row
                    .try_get("created_at")
                    .map_err(|err| backend("list", err))?,
                updated_at: row
                    .try_get("updated_at")
                    .map_err(|err| backend("list", err))?,
                last_seq: to_u64(last_seq, "last_seq")?,
                last_turn_id: TurnId(to_u64(last_turn_id, "last_turn_id")?),
            });
        }

        Ok(summaries)
    }

    async fn delete(&self, id: SessionId) -> Result<(), StoreError> {
        let result = sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(id.as_uuid())
            .execute(&self.pool)
            .await
            .map_err(|err| backend("delete", err))?;

        if result.rows_affected() == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn append(
        &self,
        id: SessionId,
        events: &[SessionEventKind],
    ) -> Result<Vec<SessionEvent>, StoreError> {
        if events.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| backend("append", err))?;

        let row = sqlx::query("SELECT last_seq, stage FROM sessions WHERE id = $1 FOR UPDATE")
            .bind(id.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(|err| backend("append", err))?;

        let Some(row) = row else {
            return Err(StoreError::NotFound);
        };

        let last_seq: i64 = row
            .try_get("last_seq")
            .map_err(|err| backend("append", err))?;
        let stage_text: String = row.try_get("stage").map_err(|err| backend("append", err))?;
        let current =
            Stage::from_str(&stage_text).map_err(|err| StoreError::Corrupt(err.to_string()))?;
        let next_stage = validate_batch(current, events)?;

        let first_seq = to_u64(last_seq, "last_seq")? + 1;
        let at = Utc::now();
        let mut stored = Vec::with_capacity(events.len());

        for (index, kind) in events.iter().enumerate() {
            let seq = first_seq + index as u64;
            let seq_i64 = i64::try_from(seq)
                .map_err(|_| StoreError::Backend("event seq overflow".to_owned()))?;
            let payload = serde_json::to_value(kind)
                .map_err(|err| StoreError::Backend(format!("encode event: {err}")))?;

            sqlx::query(
                "INSERT INTO session_events (session_id, seq, v, at, kind) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(id.as_uuid())
            .bind(seq_i64)
            .bind(EVENT_VERSION as i32)
            .bind(at)
            .bind(payload)
            .execute(&mut *tx)
            .await
            .map_err(|err| backend("append", err))?;

            stored.push(SessionEvent {
                v: EVENT_VERSION,
                seq,
                at,
                kind: kind.clone(),
            });
        }

        let new_last_seq = first_seq + events.len() as u64 - 1;
        let new_last_seq = i64::try_from(new_last_seq)
            .map_err(|_| StoreError::Backend("event seq overflow".to_owned()))?;
        let max_turn =
            max_turn_id(events).map_or(0, |turn_id| i64::try_from(turn_id.0).unwrap_or(i64::MAX));

        let update = if let Some(next) = next_stage {
            sqlx::query(
                "UPDATE sessions SET last_seq = $2, \
                 last_turn_id = GREATEST(last_turn_id, $3), updated_at = $4, stage = $5 \
                 WHERE id = $1",
            )
            .bind(id.as_uuid())
            .bind(new_last_seq)
            .bind(max_turn)
            .bind(at)
            .bind(next.as_str())
        } else {
            sqlx::query(
                "UPDATE sessions SET last_seq = $2, \
                 last_turn_id = GREATEST(last_turn_id, $3), updated_at = $4 \
                 WHERE id = $1",
            )
            .bind(id.as_uuid())
            .bind(new_last_seq)
            .bind(max_turn)
            .bind(at)
        };

        update
            .execute(&mut *tx)
            .await
            .map_err(|err| backend("append", err))?;

        tx.commit().await.map_err(|err| backend("append", err))?;
        Ok(stored)
    }
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(err) if err.is_unique_violation())
}

fn to_u64(value: i64, field: &str) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::Corrupt(format!("{field} is negative: {value}")))
}

fn backend(operation: &str, err: impl std::fmt::Display) -> StoreError {
    StoreError::Backend(format!("{operation}: {err}"))
}
