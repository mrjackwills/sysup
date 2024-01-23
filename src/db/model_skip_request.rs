use serde::Deserialize;
use sqlx::SqlitePool;
use std::fmt;

use crate::app_error::AppError;

#[derive(sqlx::FromRow, Debug, Clone, Deserialize)]
pub struct ModelSkipRequest {
    pub skip_request_id: i64,
    pub skip: bool,
}

impl fmt::Display for ModelSkipRequest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "skip_request_id: {}, skip: {}",
            self.skip_request_id, self.skip,
        )
    }
}

// impl Default for ModelSkipRequest {
//     fn default() -> Self {
//         Self {
//             skip_request_id: 1,
//             skip: true,
//         }
//     }
// }

impl ModelSkipRequest {
    pub async fn get(db: &SqlitePool) -> Option<Self> {
        let sql = "SELECT * FROM skip_request";
        let result = sqlx::query_as::<_, Self>(sql).fetch_one(db).await;
        result.ok()
    }

    pub async fn insert(db: &SqlitePool, skip: bool) -> Result<Self, AppError> {
        let sql = "INSERT INTO skip_request (skip) VALUES($1) RETURNING skip_request_id, skip";
        let query = sqlx::query_as::<_, Self>(sql)
            .bind(skip)
            .fetch_one(db)
            .await?;
        Ok(query)
    }

    pub async fn update(db: &SqlitePool, skip: bool) -> Result<Self, AppError> {
        let sql = "UPDATE skip_request SET skip = $1 RETURNING skip_request_id, skip";
        let query = sqlx::query_as::<_, Self>(sql)
            .bind(skip)
            .fetch_one(db)
            .await?;
        Ok(query)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::db::{create_tables, get_db};
    use crate::tests::{gen_app_envs, setup_test, test_cleanup};
    use uuid::Uuid;

    use super::*;

    #[tokio::test]
    async fn model_skip_get_empty_with_init() {
        let uuid = Uuid::new_v4();
        let app_envs = gen_app_envs(uuid);

        // file_exists(&app_envs.location_sqlite);
        let db = get_db(&app_envs).await.unwrap();
        create_tables(&db).await;

        
        let result = ModelSkipRequest::get(&db).await;

        
        assert!(result.is_none());
        db.close().await;
        test_cleanup(uuid, None).await;
    }

    #[tokio::test]
    async fn model_skip_insert_ok() {
        let uuid = Uuid::new_v4();
        let app_envs = gen_app_envs(uuid);
        // file_exists(&app_envs.location_sqlite);
        let db = get_db(&app_envs).await.unwrap();
        create_tables(&db).await;

        
        let result = ModelSkipRequest::insert(&db, true).await;

        
        assert!(result.is_ok());
        let result = ModelSkipRequest::get(&db).await.unwrap();
        assert!(result.skip);
        db.close().await;
        test_cleanup(uuid, None).await;
    }

    #[tokio::test]
    async fn model_skip_get_ok_with_init() {
        let (_app_envs, db, uuid) = setup_test().await;

        
        let result = ModelSkipRequest::get(&db).await;

        
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.skip);
        test_cleanup(uuid, Some(db)).await;
    }

    #[tokio::test]
    async fn model_skip_update_ok() {
        let (_app_envs, db, uuid) = setup_test().await;

        let result = ModelSkipRequest::get(&db).await.unwrap();
        assert!(result.skip);
        assert_eq!(result.skip_request_id, 1);

        
        let result = ModelSkipRequest::update(&db, false).await;
        assert!(result.is_ok());
        let result = result.unwrap();

        assert!(!result.skip);
        assert_eq!(result.skip_request_id, 1);

        let result = ModelSkipRequest::get(&db).await.unwrap();
        assert!(!result.skip);
        assert_eq!(result.skip_request_id, 1);
        test_cleanup(uuid, Some(db)).await;
    }
}
