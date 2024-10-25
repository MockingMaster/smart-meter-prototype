#[cfg(test)]
mod tests {
    use crate::{
        mock::MockDatabase, Bill, Client, DatabaseError, DatabaseInterface, DbResult, Reading,
    };

    const CLIENT_ID: &str = "id";
    async fn db_with_client(client_id: &str) -> DbResult<MockDatabase> {
        let db = MockDatabase::new();

        let client = Client {
            token: "".to_string(),
            bills: vec![],
            readings: vec![],
        };

        db.add_client(client_id.to_string(), client).await?;
        Ok(db)
    }

    #[tokio::test]
    async fn test_add_client() -> DbResult<()> {
        let db = db_with_client(CLIENT_ID).await?;
        assert!(db.client_exists(CLIENT_ID).await?.is_some());

        let client = Client {
            token: "".to_string(),
            bills: vec![],
            readings: vec![],
        };

        assert_eq!(
            db.add_client(CLIENT_ID.to_string(), client).await,
            Err(DatabaseError::DataConflict)
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_client() -> DbResult<()> {
        let db = db_with_client(CLIENT_ID).await?;

        assert_eq!(db.remove_client(CLIENT_ID).await, Ok(()));
        assert!(db.client_exists(CLIENT_ID).await?.is_none());

        assert_eq!(
            db.remove_client(CLIENT_ID).await,
            Err(DatabaseError::ClientNotFound)
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_add_reading() -> DbResult<()> {
        let db = db_with_client(CLIENT_ID).await?;
        let reading = Reading::from(10.0);

        assert_eq!(db.add_reading(CLIENT_ID, reading.clone()).await, Ok(()));
        assert!(db.last_reading(CLIENT_ID).await?.is_some());

        let clients = db.clients.read().await;
        let client = clients
            .get(CLIENT_ID)
            .expect("something went terribly wrong")
            .lock()
            .await;

        let reading2 = client.readings.last();
        assert!(reading2.is_some());
        assert_eq!(reading2.unwrap().reading, reading.reading);
        assert_eq!(reading2.unwrap().time, reading.time);

        Ok(())
    }

    #[tokio::test]
    async fn test_add_bill() -> DbResult<()> {
        let db = db_with_client(CLIENT_ID).await?;
        let reading = Reading::from(10.0);
        let bill = Bill::from_reading(&reading, 0.2, 0.4).expect("date overflow");

        assert_eq!(db.add_bill(CLIENT_ID, bill.clone()).await, Ok(()));

        let clients = db.clients.read().await;
        let client = clients
            .get(CLIENT_ID)
            .expect("something went terribly wrong")
            .lock()
            .await;

        assert!(client.bills.last().is_some());
        assert_eq!(*client.bills.last().unwrap(), bill);

        Ok(())
    }
}
