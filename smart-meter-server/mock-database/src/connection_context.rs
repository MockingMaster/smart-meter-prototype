use std::sync::Arc;

use chrono::Timelike;

use crate::{Bill, BillingPeriod, DatabaseError, DatabaseInterface, DbResult, Reading};

pub struct ConnectionContext {
    client_id: String,
    current_reading: Reading,
    current_bill: Bill,
    daily_standing_charge: f64,
    price_per_unit: f64,
    flushed: bool,
    database: Arc<dyn DatabaseInterface + Send + Sync>,
}

impl ConnectionContext {
    pub async fn new(
        client_id: String,
        price_per_unit: f64,
        daily_standing_charge: f64,
        database: Arc<dyn DatabaseInterface + Send + Sync>,
    ) -> DbResult<Self> {
        let last_reading = database
            .last_reading(&client_id)
            .await?
            .ok_or(DatabaseError::MissingReading)?;
        let last_bill = database
            .last_bill(&client_id)
            .await?
            .ok_or(DatabaseError::BillNotFound)?;
        Ok(Self {
            client_id,
            current_reading: last_reading,
            current_bill: last_bill,
            daily_standing_charge,
            price_per_unit,
            flushed: false,
            database,
        })
    }

    pub async fn add_reading(&mut self, reading: Reading) -> DbResult<()> {
        if self.current_reading.reading > reading.reading {
            return Err(DatabaseError::InvalidReading);
        }

        if reading.time.date() != self.current_reading.time.date()
            || reading.time.hour() != self.current_reading.time.hour()
        {
            /*
             * Flush readings to the database
             */
            self.database
                .add_reading(&self.client_id, reading.clone())
                .await?;

            self.update_or_create_bill(&reading).await?;
            self.flushed = true;
        } else {
            self.flushed = false;
            self.update_bill(&reading);
        }

        self.current_reading = reading;
        Ok(())
    }

    fn update_bill(&mut self, reading: &Reading) {
        self.current_bill.units_end = reading.reading;
        self.current_bill.actual_usage =
            (self.current_bill.units_end - self.current_bill.units_start) * self.price_per_unit;

        let days_elapsed =
            (reading.time.date() - self.current_bill.billing_period.start).num_days() as f64 + 1.0;
        self.current_bill.standing_charge = days_elapsed * self.daily_standing_charge;

        self.current_bill.total =
            self.current_bill.actual_usage + self.current_bill.standing_charge;
    }

    async fn update_or_create_bill(&mut self, reading: &Reading) -> DbResult<()> {
        if reading.time < self.current_bill.billing_period.end.into() {
            self.update_bill(reading);

            self.database
                .update_last_bill(&self.client_id, self.current_bill.clone())
                .await?;
        } else {
            /*
             * Reading is within a new billing period
             */
            let new_bill = self.create_new_bill(reading.clone());
            self.database
                .add_bill(&self.client_id, new_bill.clone())
                .await?;
            self.current_bill = new_bill;
        }

        Ok(())
    }

    fn create_new_bill(&self, reading: Reading) -> Bill {
        let today = reading.time;

        Bill {
            actual_usage: reading.reading,
            standing_charge: self.daily_standing_charge,
            total: reading.reading * self.price_per_unit + self.daily_standing_charge,
            units_start: reading.reading,
            units_end: reading.reading,
            price_per_unit: self.price_per_unit,
            daily_standing_charge: self.daily_standing_charge,
            billing_period: BillingPeriod {
                start: today.into(),
                end: today
                    .checked_add_months(chrono::Months::new(1))
                    .expect("date overflow")
                    .into(),
            },
        }
    }

    pub async fn flush(&mut self) -> DbResult<()> {
        if self.flushed {
            return Ok(());
        }

        self.database
            .add_reading(&self.client_id, self.current_reading.clone())
            .await?;
        self.update_or_create_bill(&self.current_reading.clone())
            .await?;
        self.flushed = true;

        Ok(())
    }

    pub fn current_reading(&self) -> Reading {
        self.current_reading.clone()
    }

    pub fn current_bill(&self) -> Bill {
        self.current_bill.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        connection_context::ConnectionContext, Bill, DbResult, MockDatabaseInterface, Reading,
    };

    const CLIENT_ID: &str = "id";
    const DAILY_STANDING_CHARGE: f64 = 0.4;
    const UNIT_COST: f64 = 0.2;
    const USAGE: f64 = 10.0;

    fn mock_db_with_client() -> MockDatabaseInterface {
        let mut db = MockDatabaseInterface::new();
        db.expect_last_reading()
            .times(1)
            .returning(|_| Ok(Some(Reading::from(USAGE))));
        db.expect_last_bill().returning(|_| {
            Ok(Bill::from_reading(
                &Reading::from(USAGE),
                UNIT_COST,
                DAILY_STANDING_CHARGE,
            ))
        });

        db
    }

    #[tokio::test]
    async fn test_create_context() -> DbResult<()> {
        let db = mock_db_with_client();
        let _ = ConnectionContext::new(
            CLIENT_ID.to_string(),
            UNIT_COST,
            DAILY_STANDING_CHARGE,
            Arc::new(db),
        )
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_add_reading_same_day() -> DbResult<()> {
        let db = mock_db_with_client();
        let mut ctx = ConnectionContext::new(
            CLIENT_ID.to_string(),
            UNIT_COST,
            DAILY_STANDING_CHARGE,
            Arc::new(db),
        )
        .await?;

        assert!(ctx.add_reading(Reading::from(5.0)).await.is_err());
        assert!(ctx.add_reading(Reading::from(15.0)).await.is_ok());
        assert!(ctx.add_reading(Reading::from(14.0)).await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_add_reading_differnt_time() -> DbResult<()> {
        let mut db = mock_db_with_client();
        db.expect_update_last_bill()
            .times(1)
            .returning(|_, _| Ok(()));
        db.expect_add_reading().times(1).returning(|_, _| Ok(()));

        let mut ctx = ConnectionContext::new(
            CLIENT_ID.to_string(),
            UNIT_COST,
            DAILY_STANDING_CHARGE,
            Arc::new(db),
        )
        .await?;

        let mut reading = Reading::from(16.0);
        reading.time = reading
            .time
            .checked_add_days(chrono::Days::new(1))
            .expect("overflow");

        assert!(ctx.add_reading(reading).await.is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn test_cache_flush() -> DbResult<()> {
        let mut db = mock_db_with_client();
        let reading = Reading::from(17.0);
        let reading2 = reading.clone();
        db.expect_update_last_bill()
            .times(1)
            .returning(|_, _| Ok(()));
        db.expect_add_reading().times(1).returning(|_, _| Ok(()));
        db.expect_last_reading()
            .times(1)
            .returning(move |_| Ok(Some(reading2)));

        let mut ctx = ConnectionContext::new(
            CLIENT_ID.to_string(),
            UNIT_COST,
            DAILY_STANDING_CHARGE,
            Arc::new(db),
        )
        .await?;

        ctx.add_reading(reading.clone()).await?;

        ctx.flush().await?;

        assert_eq!(ctx.database.last_reading(CLIENT_ID).await?, Some(reading));
        Ok(())
    }
}
