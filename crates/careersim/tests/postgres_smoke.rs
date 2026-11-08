#[tokio::test]
async fn postgres_is_reachable() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return;
    };

    let pool = sqlx::PgPool::connect(&url)
        .await
        .expect("connect to PostgreSQL");

    let value: i32 = sqlx::query_scalar("SELECT 1")
        .fetch_one(&pool)
        .await
        .expect("run SELECT 1");

    assert_eq!(value, 1);
}
