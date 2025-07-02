use std::{net::IpAddr, pin::Pin};

use futures_util::Future;
use jiff::Zoned;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use url::Url;

use crate::{
    C, S, app_env::AppEnv, app_error::AppError, db::ModelRequest, service_install::Status,
};

/// Pushover api url
const URL: &str = "https://api.pushover.net/1/messages.json";

// This shouldn't need needed, annoying clippy lint
/// What's my ipv4 url
const URL_V4: &str = "https://myipv4.p1.opendns.com/get_my_ip";
/// What's my ipv6 url
const URL_V6: &str = "https://myipv6.p1.opendns.com/get_my_ip";

type Params<'a> = [(&'a str, String); 4];

#[derive(Debug, Serialize, Deserialize)]
/// Response from pushover api, currently not actually doing anything with it
struct PostRequest {
    status: usize,
    request: String,
}

/// Response from the what's my ip api
#[derive(Debug, Serialize, Deserialize)]
struct IpResponse {
    ip: IpAddr,
}

enum Ip {
    V4,
    V6,
}

#[allow(unused)]
impl Ip {
    const fn get_url(&self) -> &'static str {
        match self {
            Self::V4 => URL_V4,
            Self::V6 => URL_V6,
        }
    }
}

pub enum PushRequest {
    Service(Status),
    Online,
}

impl From<Status> for PushRequest {
    fn from(value: Status) -> Self {
        Self::Service(value)
    }
}

impl PushRequest {
    /// Get the reqwest client, in reality should never actually fail
    fn get_client() -> Result<Client, AppError> {
        Ok(reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_millis(5000))
            .gzip(true)
            .brotli(true)
            .user_agent(format!(
                "{}/{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ))
            .build()?)
    }

    #[cfg(not(test))]
    /// Recursive function to check if network is up
    fn get_ip(
        mut count: u8,
        ip: Ip,
    ) -> Pin<Box<dyn Future<Output = Result<IpResponse, AppError>> + Send>> {
        Box::pin(async move {
            if count > 10 {
                return Err(AppError::Offline);
            }
            let client = Self::get_client()?;
            if let Ok(response) = client.get(ip.get_url()).send().await {
                Ok(response.json::<IpResponse>().await?)
            } else {
                tracing::debug!("Recursively sleeping for 500ms");
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                count += 1;
                Self::get_ip(count, ip).await
            }
        })
    }

    #[cfg(test)]
    #[expect(unused_mut)]
    /// TEst mock for ip, ipv6 issues on wsl :(
    fn get_ip(
        mut count: u8,
        ip: Ip,
    ) -> Pin<Box<dyn Future<Output = Result<IpResponse, AppError>> + Send>> {
        use std::net::{Ipv4Addr, Ipv6Addr};

        Box::pin(async move {
            if count > 10 {
                return Err(AppError::Offline);
            }

            Ok(IpResponse {
                ip: match ip {
                    Ip::V4 => IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                    Ip::V6 => IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)),
                },
            })
        })
    }

    #[cfg(not(test))]
    /// The actual request via PushOver api
    async fn send_request(url: Url) -> Result<PostRequest, AppError> {
        let client = Self::get_client()?;
        Ok(client.post(url).send().await?.json::<PostRequest>().await?)
    }

    #[cfg(test)]
    #[expect(clippy::unused_async)]
    async fn send_request(_: Url) -> Result<PostRequest, AppError> {
        let _client = Self::get_client()?;
        Ok(PostRequest {
            status: 1,
            request: S!("request"),
        })
    }

    /// Basically fmt::Display for the current time using the app_env timezone
    fn format_offset(app_envs: &AppEnv, offset: &Zoned) -> String {
        format!(
            "{} {:02}:{:02}:{:02} {}",
            offset.date(),
            offset.hour(),
            offset.minute(),
            offset.second(),
            app_envs.timezone.iana_name().unwrap_or_default()
        )
    }

    /// Generate the params, aka the message
    fn gen_params<'a>(&self, app_envs: &AppEnv, ipv4: IpAddr, ipv6: IpAddr) -> Params<'a> {
        let mut params = [
            ("token", C!(app_envs.token_app)),
            ("user", C!(app_envs.token_user)),
            ("message", S!()),
            ("priority", S!("0")),
        ];

        let suffix = format!(
            "@ {} {} {}",
            Self::format_offset(app_envs, &ModelRequest::now_with_offset(app_envs)),
            ipv4,
            ipv6
        );

        match self {
            Self::Online => {
                params[2].1 = format!("{} online {suffix}", app_envs.machine_name,);
            }
            Self::Service(status) => {
                params[2].1 = format!("{} on {} {suffix}", status.get(), app_envs.machine_name,);
            }
        }
        params
    }

    /// Make the request, will check to make sure that haven't made 6+ request in past hour
    /// get_ip functions are recursive, to deal with no network at first boot
	#[allow(clippy::cognitive_complexity)]
    pub async fn make_request(&self, app_envs: &AppEnv, db: &SqlitePool) -> Result<(), AppError> {
        let requests_made = ModelRequest::get_past_hour(db).await?;

        if requests_made.len() >= 6 {
            tracing::info!("6 Requests made in past hour, skipping sending request");
            for i in requests_made {
                tracing::info!(
                    "{}",
                    Self::format_offset(app_envs, &i.timestamp_to_offset(app_envs))
                );
            }
        } else {
            tracing::debug!("Checking network connection");
            let ipv4 = Self::get_ip(0, Ip::V4).await?.ip;
            let ipv6 = Self::get_ip(0, Ip::V6).await?.ip;

            tracing::debug!("Sending request");
            let params = self.gen_params(app_envs, ipv4, ipv6);
            let url = reqwest::Url::parse_with_params(URL, &params)?;
            ModelRequest::insert(db).await?;
            Self::send_request(url).await?;
            tracing::debug!("Request sent");
        }
        Ok(())
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {

    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::*;
    use crate::tests::{setup_test, test_cleanup};

    #[tokio::test]
    async fn test_request_format_offset() {
        let (app_envs, db, uuid) = setup_test().await;

        let result =
            PushRequest::format_offset(&app_envs, &ModelRequest::now_with_offset(&app_envs));

        let separator = |index: usize, sym: &str| {
            assert_eq!(result.chars().skip(index).take(1).collect::<String>(), sym);
        };

        let numeric = |skip: usize, take: usize| {
            assert!(result.chars().skip(skip).take(take).all(char::is_numeric));
        };

        // Year section
        numeric(0, 4);
        // 1st dash
        separator(4, "-");
        // month section
        numeric(5, 2);
        // 2nd dash
        separator(7, "-");
        // day section
        numeric(8, 2);
        // space
        separator(10, " ");
        // hour section
        numeric(11, 2);
        // 1st colon
        separator(13, ":");
        // minute section
        numeric(14, 2);
        // 2nd colon
        separator(16, ":");
        // second section
        numeric(17, 2);

        // second section
        let hour: String = result.chars().skip(17).take(2).collect::<String>();
        assert!(hour.chars().all(char::is_numeric));

        assert!(result.ends_with("Europe/London"));
        test_cleanup(uuid, Some(db)).await;
    }

    #[tokio::test]
    async fn test_request_generate_params() {
        let (app_envs, db, uuid) = setup_test().await;
        let ipv4 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let ipv6 = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));

        let push_request = PushRequest::Online;
        let result = push_request.gen_params(&app_envs, ipv4, ipv6);

        // This will fail when the utc/london timezones aren't in sync
        assert_eq!(result[0], ("token", S!("test_token_app")));

        assert_eq!(result[2].0, "message");
        assert!(result[2].1.starts_with("test_machine online @ 20"));
        assert!(result[2].1.contains(" Europe/London 127.0.0.1 ::1"));

        assert_eq!(result[1], ("user", S!("test_token_user")));

        assert_eq!(result[3], ("priority", S!("0")));

        let push_request = PushRequest::Service(Status::Install);
        let result = push_request.gen_params(&app_envs, ipv4, ipv6);

        assert_eq!(result[0], ("token", S!("test_token_app")));
        assert_eq!(result[2].0, "message");
        assert!(
            result[2]
                .1
                .starts_with("service installed on test_machine @ 20")
        );
        assert!(result[2].1.contains(" Europe/London 127.0.0.1 ::1"));
        assert_eq!(result[1], ("user", S!("test_token_user")));
        assert_eq!(result[3], ("priority", S!("0")));

        let push_request = PushRequest::Service(Status::Uninstall);
        let result = push_request.gen_params(&app_envs, ipv4, ipv6);

        assert_eq!(result[0], ("token", S!("test_token_app")));
        assert_eq!(result[2].0, "message");
        assert!(
            result[2]
                .1
                .starts_with("service uninstalled on test_machine @ 20")
        );
        assert!(result[2].1.contains(" Europe/London 127.0.0.1 ::1"));
        assert_eq!(result[1], ("user", S!("test_token_user")));
        assert_eq!(result[3], ("priority", S!("0")));

        test_cleanup(uuid, Some(db)).await;
    }

    #[tokio::test]
    // Request not made if 6+ requests been made in previous 60 minutes
    async fn test_request_make_request_not_made() {
        let (app_envs, db, uuid) = setup_test().await;

        let now = i64::try_from(ModelRequest::now()).unwrap();
        for i in 1..=6 {
            let sql = "INSERT INTO request(timestamp) VALUES ($1) RETURNING request_id, timestamp";
            let timestamp = now - (60 * (i * 2));

            sqlx::query_as::<_, ModelRequest>(sql)
                .bind(timestamp)
                .fetch_one(&db)
                .await
                .unwrap();
        }

        let request_len = ModelRequest::get_all(&db).await;
        assert!(request_len.is_ok());
        assert_eq!(request_len.unwrap().len(), 6);

        let result = PushRequest::Online.make_request(&app_envs, &db).await;

        assert!(result.is_ok());

        let request_len = ModelRequest::get_all(&db).await;
        assert!(request_len.is_ok());
        assert_eq!(request_len.unwrap().len(), 6);

        test_cleanup(uuid, Some(db)).await;
    }

    #[tokio::test]
    // Request made, and inserted into db
    async fn test_request_get_ip_count() {
        let result = PushRequest::get_ip(11, Ip::V4).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    // Request made, and inserted into db
    async fn test_request_make_request_count() {}

    #[tokio::test]
    // Request made, and inserted into db
    async fn test_request_make_request() {
        let (app_envs, db, uuid) = setup_test().await;

        let request_len = ModelRequest::get_all(&db).await;
        assert!(request_len.is_ok());
        assert_eq!(request_len.unwrap().len(), 0);

        let result = PushRequest::Online.make_request(&app_envs, &db).await;

        assert!(result.is_ok());

        let request_len = ModelRequest::get_all(&db).await;
        assert!(request_len.is_ok());
        assert_eq!(request_len.unwrap().len(), 1);

        test_cleanup(uuid, Some(db)).await;
    }
}
