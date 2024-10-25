use std::sync::Arc;

use async_trait::async_trait;
use chrono::{NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod connection_context;
pub mod mock;

pub type Database = Arc<dyn DatabaseInterface + Send + Sync>;

pub struct Client {
    pub token: String,
    pub bills: Vec<Bill>,
    pub readings: Vec<Reading>,
}

#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Debug)]
pub struct Reading {
    pub reading: f64,
    pub time: NaiveDateTime,
}

impl From<f64> for Reading {
    fn from(value: f64) -> Self {
        Reading {
            reading: value,
            time: Utc::now().naive_utc(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BillingPeriod {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl BillingPeriod {
    pub fn start(&self) -> NaiveDate {
        self.start
    }

    pub fn end(&self) -> NaiveDate {
        self.end
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Bill {
    pub actual_usage: f64,
    pub standing_charge: f64,
    pub total: f64,
    pub units_start: f64,
    pub units_end: f64,
    pub price_per_unit: f64,
    pub daily_standing_charge: f64,
    pub billing_period: BillingPeriod,
}

impl Bill {
    pub fn from_reading(
        reading: &Reading,
        price_per_unit: f64,
        daily_standing_charge: f64,
    ) -> Option<Self> {
        let end = match reading.time.checked_add_months(chrono::Months::new(1)) {
            Some(end) => end,
            None => return None,
        };

        let billing_period = BillingPeriod {
            start: reading.time.into(),
            end: end.into(),
        };

        /*
         * create a new bill, set standing charge to 1 day
         */
        Some(Bill {
            actual_usage: price_per_unit * reading.reading,
            standing_charge: daily_standing_charge,
            total: (reading.reading * price_per_unit) + daily_standing_charge,
            units_end: reading.reading,
            price_per_unit,
            daily_standing_charge,
            units_start: 0.0,
            billing_period,
        })
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum DatabaseError {
    #[error("Client not found")]
    ClientNotFound,
    #[error("Bill not found")]
    BillNotFound,
    #[error("Database connection error")]
    ConnectionError,
    #[error("Data conflict or integrity issue")]
    DataConflict,
    #[error("Reading is smaller than previous reading")]
    InvalidReading,
    #[error("Client does not have any readings")]
    MissingReading,
    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type DbResult<T> = Result<T, DatabaseError>;

use mockall::{automock, predicate::*};

#[automock]
#[async_trait]
pub trait DatabaseInterface {
    async fn add_client(&self, client_id: String, client: Client) -> DbResult<()>;
    async fn remove_client(&self, client_id: &str) -> DbResult<()>;
    async fn add_reading(&self, client_id: &str, reading: Reading) -> DbResult<()>;
    async fn add_bill(&self, client_id: &str, bill: Bill) -> DbResult<()>;
    async fn last_bill(&self, client_id: &str) -> DbResult<Option<Bill>>;
    async fn last_reading(&self, client_id: &str) -> DbResult<Option<Reading>>;
    async fn update_last_bill(&self, client_id: &str, bill: Bill) -> DbResult<()>;
    async fn client_exists(&self, client_id: &str) -> DbResult<Option<String>>;
}
