use jiff::{SpanRound, ToSpan, Unit, Zoned};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::time::Duration;
use std::{fmt, time::SystemTime};

use crate::app_error::AppError;
use crate::{C, app_env::AppEnv};

#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModelRequest {
    pub request_id: i64,
    #[sqlx(try_from = "i64")]
    pub timestamp: u64,
}

impl fmt::Display for ModelRequest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "request_id: {}, timestamp:{}",
            self.request_id, self.timestamp,
        )
    }
}

impl ModelRequest {
    /// Get the current time in seconds, unix epoch style
    pub fn now() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    pub fn now_with_offset(app_envs: &AppEnv) -> jiff::Zoned {
        jiff::Timestamp::now().to_zoned(C!(app_envs.timezone))
    }

    pub fn timestamp_to_offset(&self, app_envs: &AppEnv) -> Zoned {
        Self::now_with_offset(app_envs).saturating_add(Duration::from_secs(self.timestamp))
    }

    #[cfg(test)]
    pub async fn get_all(db: &SqlitePool) -> Result<Vec<Self>, AppError> {
        let sql = "SELECT * FROM request";
        let result = sqlx::query_as::<_, Self>(sql).fetch_all(db).await?;
        Ok(result)
    }

    /// Get all request made in the last hour
    pub async fn get_past_hour(db: &SqlitePool) -> Result<Vec<Self>, AppError> {
        let sql = "SELECT * FROM request WHERE timestamp BETWEEN $1 AND $2 ORDER BY timestamp";
        let now = i64::try_from(Self::now())?;
        let one_hour = 1
            .hour()
            .round(SpanRound::new().largest(Unit::Second))
            .map_or(0, |i| i.get_seconds());
        let result = sqlx::query_as::<_, Self>(sql)
            .bind(now - one_hour)
            .bind(now)
            .fetch_all(db)
            .await?;
        Ok(result)
    }

    // insert a new request with timestamp
    pub async fn insert(db: &SqlitePool) -> Result<Self, AppError> {
        let sql = "INSERT INTO request(timestamp) VALUES ($1) RETURNING request_id, timestamp";
        let query = sqlx::query_as::<_, Self>(sql)
            .bind(i64::try_from(Self::now())?)
            .fetch_one(db)
            .await?;
        Ok(query)
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {

    use crate::tests::{setup_test, test_cleanup};

    use super::*;

    #[tokio::test]
    async fn model_request_add_ok() {
        let (_app_envs, db, uuid) = setup_test().await;

        let now = ModelRequest::now();
        let result = ModelRequest::insert(&db).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.request_id, 1);
        assert_eq!(result.timestamp, now);
        test_cleanup(uuid, Some(db)).await;
    }

    #[tokio::test]
    async fn model_request_offset() {
        let (_app_envs, db, uuid) = setup_test().await;

        let now = ModelRequest::now();
        let result = ModelRequest::insert(&db).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.request_id, 1);
        assert_eq!(result.timestamp, now);
        test_cleanup(uuid, Some(db)).await;
    }

    #[tokio::test]
    async fn model_request_get_all_ok() {
        let (_app_envs, db, uuid) = setup_test().await;
        let now = ModelRequest::now();
        for i in 0..4 {
            let sql = "INSERT INTO request(timestamp) VALUES ($1) RETURNING request_id, timestamp";
            sqlx::query_as::<_, ModelRequest>(sql)
                .bind(i64::try_from(now + i).unwrap())
                .fetch_one(&db)
                .await
                .unwrap();
        }

        let result = ModelRequest::get_all(&db).await;

        assert!(result.is_ok());
        let result = result.unwrap();

        assert_eq!(result.len(), 4);
        assert_eq!(result[0].timestamp, now);
        assert_eq!(result[0].request_id, 1);

        assert_eq!(result[1].timestamp, now + 1);
        assert_eq!(result[1].request_id, 2);

        assert_eq!(result[2].timestamp, now + 2);
        assert_eq!(result[2].request_id, 3);

        assert_eq!(result[3].timestamp, now + 3);
        assert_eq!(result[3].request_id, 4);

        test_cleanup(uuid, Some(db)).await;
    }

    #[tokio::test]
    async fn model_request_get_last_hour_ok() {
        let (_app_envs, db, uuid) = setup_test().await;

        let now = i64::try_from(ModelRequest::now()).unwrap();
        for i in 1..=4 {
            let sql = "INSERT INTO request(timestamp) VALUES ($1) RETURNING request_id, timestamp";
            let timestamp = now - (60 * (i * 25));

            sqlx::query_as::<_, ModelRequest>(sql)
                .bind(timestamp)
                .fetch_one(&db)
                .await
                .unwrap();
        }

        let result = ModelRequest::get_past_hour(&db).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.len(), 2);

        let expected = vec![
            ModelRequest {
                request_id: 2,
                timestamp: u64::try_from(now - (60 * 50)).unwrap(),
            },
            ModelRequest {
                request_id: 1,
                timestamp: u64::try_from(now - (60 * 25)).unwrap(),
            },
        ];

        assert_eq!(result, expected);
        test_cleanup(uuid, Some(db)).await;
    }
}
