use crate::models::FloodDisplay;
use chrono::{Duration, Utc};
use chrono_tz::US::Pacific;
use noaa_tides::products::predictions::TideType;
use noaa_tides::{NoaaTideClient, PredictionsRequest, params};
use sqlx::sqlite::SqlitePool;

const STATION_ID: &str = "9414819";
pub const FLOOD_THRESHOLD_FT: f64 = 6.4;
pub const FORECAST_DAYS: i64 = 30;

pub async fn update_tide_predictions(pool: SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let client = NoaaTideClient::new();
    let begin_date = Utc::now().with_timezone(&Pacific).date_naive();
    let end_date = begin_date + Duration::days(FORECAST_DAYS);

    let request = PredictionsRequest {
        station: STATION_ID.into(),
        date_range: params::DateRange {
            begin_date,
            end_date,
        },
        datum: params::Datum::MLLW,
        time_zone: params::Timezone::LST_LDT,
        interval: params::Interval::HighLow,
        units: params::Units::English,
    };

    let predictions = client.fetch_predictions(&request).await?.predictions;

    // Drop existing predictions in case of updates
    let begin_time = begin_date.and_hms_opt(0, 0, 0).unwrap();
    let end_time = end_date.and_hms_opt(23, 59, 59).unwrap();

    let mut tx = pool.begin().await?;
    sqlx::query!(
        r#"
        DELETE FROM tides
        WHERE prediction_time >= ? AND prediction_time <= ?;
        "#,
        begin_time,
        end_time,
    )
    .execute(&mut *tx)
    .await?;
    let mut query_builder =
        sqlx::QueryBuilder::new("INSERT INTO tides (prediction_time, height_ft, tide_type) ");

    query_builder.push_values(
        predictions.iter().filter(|p| p.tide_type.is_some()),
        |mut b, prediction| {
            let tide_type = match prediction.tide_type {
                Some(TideType::High) => "High",
                Some(TideType::Low) => "Low",
                _ => unreachable!(),
            };
            b.push_bind(prediction.datetime)
                .push_bind(prediction.height)
                .push_bind(tide_type);
        },
    );

    query_builder.build().execute(&mut *tx).await?;
    tx.commit().await?;

    println!("Successfully updated {} rows.", predictions.len());
    Ok(())
}

pub async fn get_flood_predictions(
    pool: &SqlitePool,
    check_time: chrono::DateTime<Utc>,
) -> Result<Vec<FloodDisplay>, Box<dyn std::error::Error>> {
    let local_check_time = check_time.with_timezone(&Pacific).naive_local();

    let predictions = sqlx::query!(
        r#"
        SELECT prediction_time, height_ft
        FROM tides
        WHERE prediction_time >= ? AND height_ft >= ?
        ORDER BY prediction_time ASC
        "#,
        local_check_time,
        FLOOD_THRESHOLD_FT,
    )
    .fetch_all(pool)
    .await?;

    let results = predictions
        .into_iter()
        .map(|record| FloodDisplay::new(record.prediction_time, record.height_ft))
        .collect();

    Ok(results)
}
