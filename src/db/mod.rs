mod model_request;
mod model_skip_request;

pub use model_request::ModelRequest;
pub use model_skip_request::ModelSkipRequest;

use sqlx::{ConnectOptions, SqlitePool, sqlite::SqliteJournalMode};

use crate::{Code, app_env::AppEnv, exit};

/// Open Sqlite pool connection, and return
/// `max_connections` need to be 1, [see issue](https://github.com/launchbadge/sqlx/issues/816)
async fn get_db(app_envs: &AppEnv) -> Result<SqlitePool, sqlx::Error> {
    let mut connect_options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&app_envs.location_sqlite)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);

    match app_envs.log_level {
        tracing::Level::TRACE | tracing::Level::DEBUG => (),
        _ => connect_options = connect_options.disable_statement_logging(),
    }

    let db = sqlx::pool::PoolOptions::<sqlx::Sqlite>::new()
        .max_connections(1)
        .connect_with(connect_options)
        .await?;
    Ok(db)
}

/// check if skip_request flag if not then insert, default to true
async fn insert_skip_request(db: &SqlitePool) {
    if ModelSkipRequest::get(db).await.is_none() {
        ModelSkipRequest::insert(db, true).await.ok();
    }
}

async fn create_tables(db: &SqlitePool) {
    let init_db = include_str!("init_db.sql");
    match sqlx::query(init_db).execute(db).await {
        Ok(_) => (),
        Err(e) => {
            let err = format!("create_table::{e}");
            exit(&err, &Code::Invalid);
        }
    }
}

/// Init db connection, works if folder/files exists or not
pub async fn init_db(app_envs: &AppEnv) -> Result<SqlitePool, sqlx::Error> {
    let db = get_db(app_envs).await?;
    create_tables(&db).await;
    insert_skip_request(&db).await;
    Ok(db)
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use uuid::Uuid;

    use std::fs;

    use super::*;
    use crate::tests::{gen_app_envs, test_cleanup};

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn sql_mod_db_created() {
        let uuid = Uuid::new_v4();
        let args = gen_app_envs(uuid);

        // ACTION
        let db = init_db(&args).await.unwrap();

        let sql_name = format!("/dev/shm/{uuid}.db");
        let sql_sham = format!("{sql_name}-shm");
        let sql_wal = format!("{sql_name}-wal");

        assert!(fs::exists(sql_name).unwrap_or_default());
        assert!(fs::exists(sql_sham).unwrap_or_default());
        assert!(fs::exists(sql_wal).unwrap_or_default());

        db.close().await;
        // CLEANUP
        test_cleanup(uuid, None).await;
    }

    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn sql_mod_db_created() {
        let uuid = Uuid::new_v4();
        let args = gen_app_envs(uuid);

        // ACTION
        let db = init_db(&args).await.unwrap();

        let sql_name = format!("./windows_tests/{uuid}.db");
        let sql_sham = format!("{sql_name}-shm");
        let sql_wal = format!("{sql_name}-wal");

        assert!(fs::exists(sql_name)unwrap_or_default());
        assert!(fs::exists(sql_sham)unwrap_or_default());
        assert!(fs::exists(sql_wal)unwrap_or_default());

        db.close().await;
        // CLEANUP
        test_cleanup(uuid, None).await;
    }

    #[tokio::test]
    // By default, database will have skip=true set
    async fn sql_mod_db_created_with_skip() {
        let uuid = Uuid::new_v4();
        let args = gen_app_envs(uuid);

        init_db(&args).await.unwrap();
        let db = sqlx::pool::PoolOptions::<sqlx::Sqlite>::new()
            .max_connections(1)
            .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&args.location_sqlite))
            .await
            .unwrap();

        // ACTION
        let result = sqlx::query_as("SELECT * FROM skip_request")
            .fetch_one(&db)
            .await;

        assert!(result.is_ok());
        let result: (i64, bool) = result.unwrap();
        assert_eq!(result.0, 1);
        assert!(result.1);

        // CLEANUP
        db.close().await;
        test_cleanup(uuid, None).await;
    }
}
