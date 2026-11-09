//! PostgreSQL 存储实现。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::AppendBatch;
use super::EVENT_VERSION;
use super::SessionStore;
use super::StoreError;
use super::StoredEvent;
use super::StoredSession;
use super::backend;
use super::to_u64;

pub(crate) struct PostgresSessionStore {
    pool: PgPool,
}

impl PostgresSessionStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn insert_session(
        &self,
        id: Uuid,
        stage: &str,
        at: DateTime<Utc>,
        meta: Value,
    ) -> Result<(), StoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| backend("create", err))?;

        sqlx::query(
            "INSERT INTO sessions (id, stage, created_at, updated_at, last_seq, last_turn_id) \
             VALUES ($1, $2, $3, $3, 0, 0)",
        )
        .bind(id)
        .bind(stage)
        .bind(at)
        .execute(&mut *tx)
        .await
        .map_err(|err| {
            if is_unique_violation(&err) {
                StoreError::Conflict
            } else {
                backend("create", err)
            }
        })?;

        sqlx::query(
            "INSERT INTO session_events (session_id, seq, v, at, kind) \
             VALUES ($1, 0, $2, $3, $4)",
        )
        .bind(id)
        .bind(EVENT_VERSION as i32)
        .bind(at)
        .bind(meta)
        .execute(&mut *tx)
        .await
        .map_err(|err| backend("create", err))?;

        tx.commit().await.map_err(|err| backend("create", err))
    }
}

#[async_trait]
impl SessionStore for PostgresSessionStore {
    async fn create(
        &self,
        id: Uuid,
        stage: &str,
        meta: Value,
    ) -> Result<(StoredSession, StoredEvent), StoreError> {
        let at = Utc::now();
        self.insert_session(id, stage, at, meta.clone()).await?;

        let session = StoredSession {
            id,
            stage: stage.to_owned(),
            created_at: at,
            updated_at: at,
            last_seq: 0,
            last_turn_id: 0,
        };
        let event = StoredEvent {
            v: EVENT_VERSION,
            seq: 0,
            at,
            kind: meta,
        };
        Ok((session, event))
    }

    async fn load(&self, id: Uuid) -> Result<(StoredSession, Vec<StoredEvent>), StoreError> {
        let mut tx = self
            .pool
            .begin_with("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .await
            .map_err(|err| backend("load", err))?;

        let row = sqlx::query(
            "SELECT id, stage, created_at, updated_at, last_seq, last_turn_id \
             FROM sessions WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| backend("load", err))?;

        let Some(row) = row else {
            return Err(StoreError::NotFound);
        };

        let row_id: Uuid = row.try_get("id").map_err(|err| backend("load", err))?;
        let stage: String = row.try_get("stage").map_err(|err| backend("load", err))?;
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

        if row_id != id {
            return Err(StoreError::Corrupt(
                "session row id does not match the requested id".to_owned(),
            ));
        }

        let session = StoredSession {
            id,
            stage,
            created_at,
            updated_at,
            last_seq: to_u64(last_seq, "last_seq")?,
            last_turn_id: to_u64(last_turn_id, "last_turn_id")?,
        };

        let rows = sqlx::query(
            "SELECT seq, v, at, kind FROM session_events \
             WHERE session_id = $1 ORDER BY seq ASC",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|err| backend("load", err))?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let seq: i64 = row.try_get("seq").map_err(|err| backend("load", err))?;
            let v: i32 = row.try_get("v").map_err(|err| backend("load", err))?;
            let at: DateTime<Utc> = row.try_get("at").map_err(|err| backend("load", err))?;
            let kind: Value = row.try_get("kind").map_err(|err| backend("load", err))?;

            let seq = to_u64(seq, "seq")?;
            let v = u32::try_from(v)
                .map_err(|_| StoreError::Corrupt(format!("negative event version at seq {seq}")))?;

            events.push(StoredEvent { v, seq, at, kind });
        }

        tx.commit().await.map_err(|err| backend("load", err))?;

        Ok((session, events))
    }

    async fn list(&self, limit: usize) -> Result<Vec<StoredSession>, StoreError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = sqlx::query(
            "SELECT id, stage, created_at, updated_at, last_seq, last_turn_id \
             FROM sessions ORDER BY updated_at DESC, id DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| backend("list", err))?;

        let mut sessions = Vec::with_capacity(rows.len());
        for row in rows {
            let id: Uuid = row.try_get("id").map_err(|err| backend("list", err))?;
            let stage: String = row.try_get("stage").map_err(|err| backend("list", err))?;
            let last_seq: i64 = row
                .try_get("last_seq")
                .map_err(|err| backend("list", err))?;
            let last_turn_id: i64 = row
                .try_get("last_turn_id")
                .map_err(|err| backend("list", err))?;

            sessions.push(StoredSession {
                id,
                stage,
                created_at: row
                    .try_get("created_at")
                    .map_err(|err| backend("list", err))?,
                updated_at: row
                    .try_get("updated_at")
                    .map_err(|err| backend("list", err))?,
                last_seq: to_u64(last_seq, "last_seq")?,
                last_turn_id: to_u64(last_turn_id, "last_turn_id")?,
            });
        }

        Ok(sessions)
    }

    async fn delete(&self, id: Uuid) -> Result<(), StoreError> {
        let result = sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(id)
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
        id: Uuid,
        batch: AppendBatch<'_>,
    ) -> Result<Vec<StoredEvent>, StoreError> {
        if batch.events.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| backend("append", err))?;

        let row = sqlx::query(
            "SELECT last_seq, last_turn_id, stage FROM sessions WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| backend("append", err))?;

        let Some(row) = row else {
            return Err(StoreError::NotFound);
        };

        let last_seq: i64 = row
            .try_get("last_seq")
            .map_err(|err| backend("append", err))?;
        let last_turn_id: i64 = row
            .try_get("last_turn_id")
            .map_err(|err| backend("append", err))?;
        let stage: String = row.try_get("stage").map_err(|err| backend("append", err))?;

        let current_last_seq = to_u64(last_seq, "last_seq")?;
        if stage != batch.expected.stage || current_last_seq != batch.expected.last_seq {
            return Err(StoreError::Conflict);
        }

        let first_seq = current_last_seq + 1;
        let at = Utc::now();
        let mut stored = Vec::with_capacity(batch.events.len());

        for (index, kind) in batch.events.iter().enumerate() {
            let seq = first_seq + index as u64;
            let seq_i64 = i64::try_from(seq)
                .map_err(|_| StoreError::Backend("event seq overflow".to_owned()))?;

            sqlx::query(
                "INSERT INTO session_events (session_id, seq, v, at, kind) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(id)
            .bind(seq_i64)
            .bind(EVENT_VERSION as i32)
            .bind(at)
            .bind(kind)
            .execute(&mut *tx)
            .await
            .map_err(|err| backend("append", err))?;

            stored.push(StoredEvent {
                v: EVENT_VERSION,
                seq,
                at,
                kind: kind.clone(),
            });
        }

        let new_last_seq = first_seq + batch.events.len() as u64 - 1;
        let new_last_seq = i64::try_from(new_last_seq)
            .map_err(|_| StoreError::Backend("event seq overflow".to_owned()))?;
        let new_last_turn_id =
            i64::try_from(to_u64(last_turn_id, "last_turn_id")?.max(batch.last_turn_id))
                .unwrap_or(i64::MAX);

        let update = if let Some(next) = batch.next_stage {
            sqlx::query(
                "UPDATE sessions SET last_seq = $2, \
                 last_turn_id = $3, updated_at = $4, stage = $5 \
                 WHERE id = $1",
            )
            .bind(id)
            .bind(new_last_seq)
            .bind(new_last_turn_id)
            .bind(at)
            .bind(next)
        } else {
            sqlx::query(
                "UPDATE sessions SET last_seq = $2, \
                 last_turn_id = $3, updated_at = $4 \
                 WHERE id = $1",
            )
            .bind(id)
            .bind(new_last_seq)
            .bind(new_last_turn_id)
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
