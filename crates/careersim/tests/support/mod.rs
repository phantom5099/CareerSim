use std::{path::PathBuf, thread};

use careersim::{account::model::UserId, agent::session::SessionId};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

/// 集成测试创建的数据清理守卫；即使测试 panic，也会删除本用例拥有的行。
pub struct CleanupGuard {
    database_url: String,
    user_ids: Vec<Uuid>,
    session_ids: Vec<Uuid>,
    directories: Vec<PathBuf>,
}

// 各集成测试二进制使用共享守卫 API 的不同子集。
#[allow(dead_code)]
impl CleanupGuard {
    pub fn new(database_url: &str) -> Self {
        Self {
            database_url: database_url.to_owned(),
            user_ids: Vec::new(),
            session_ids: Vec::new(),
            directories: Vec::new(),
        }
    }

    pub fn track_user(&mut self, id: UserId) -> UserId {
        self.user_ids.push(id.into_uuid());
        id
    }

    pub fn track_session(&mut self, id: SessionId) -> SessionId {
        self.session_ids.push(id.into_uuid());
        id
    }

    pub fn track_directory(&mut self, path: PathBuf) -> PathBuf {
        self.directories.push(path.clone());
        path
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if self.user_ids.is_empty() && self.session_ids.is_empty() && self.directories.is_empty() {
            return;
        }

        let database_url = self.database_url.clone();
        let user_ids = std::mem::take(&mut self.user_ids);
        let session_ids = std::mem::take(&mut self.session_ids);
        let directories = std::mem::take(&mut self.directories);
        for directory in directories {
            let _ = std::fs::remove_dir_all(directory);
        }
        let _ = thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                eprintln!("test cleanup runtime initialization failed");
                return;
            };

            runtime.block_on(async move {
                let pool = match PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&database_url)
                    .await
                {
                    Ok(pool) => pool,
                    Err(error) => {
                        eprintln!("test cleanup could not connect: {error}");
                        return;
                    }
                };
                let mut transaction = match pool.begin().await {
                    Ok(transaction) => transaction,
                    Err(error) => {
                        eprintln!("test cleanup could not begin transaction: {error}");
                        return;
                    }
                };

                if !user_ids.is_empty()
                    && sqlx::query("DELETE FROM users WHERE id = ANY($1::uuid[])")
                        .bind(&user_ids)
                        .execute(&mut *transaction)
                        .await
                        .is_err()
                {
                    eprintln!("test cleanup could not delete users");
                    return;
                }

                if !session_ids.is_empty()
                    && sqlx::query("DELETE FROM sessions WHERE id = ANY($1::uuid[])")
                        .bind(&session_ids)
                        .execute(&mut *transaction)
                        .await
                        .is_err()
                {
                    eprintln!("test cleanup could not delete sessions");
                    return;
                }

                if transaction.commit().await.is_err() {
                    eprintln!("test cleanup could not commit transaction");
                }
            });
        })
        .join();
    }
}
