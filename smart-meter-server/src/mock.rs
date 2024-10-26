#[cfg(test)]
mod tests {
    use core::panic;
    use std::sync::Arc;

    use bytes::Bytes;
    use mock_database::{mock::MockDatabase, Bill, Client, Database, Reading};
    use tokio::sync::RwLock;
    use tokio_util::codec::{Encoder, LengthDelimitedCodec};

    struct CodecBuilder<E: Encoder<Bytes>> {
        builder: tokio_test::io::Builder,
        encoder: E,
    }

    impl<E: Encoder<Bytes>> CodecBuilder<E> {
        fn new(encoder: E) -> Self {
            CodecBuilder {
                builder: tokio_test::io::Builder::new(),
                encoder,
            }
        }

        fn read(&mut self, bytes: &[u8]) -> &mut Self {
            let mut buf = bytes::BytesMut::new();
            if let Err(_) = self.encoder.encode(Bytes::copy_from_slice(bytes), &mut buf) {
                panic!("failed to encode bytes");
            }
            self.builder.read(buf.as_ref());
            self
        }

        fn write(&mut self, bytes: &[u8]) -> &mut Self {
            let mut buf = bytes::BytesMut::new();
            if let Err(_) = self.encoder.encode(Bytes::copy_from_slice(bytes), &mut buf) {
                panic!("failed to encode bytes");
            }
            self.builder.write(buf.as_ref());
            self
        }

        fn build(&mut self) -> tokio_test::io::Mock {
            self.builder.build()
        }
    }

    use crate::{handle_client, store::Store, Auth, ClientMessage, ServerMessage};

    const UNIT_COST: f64 = 0.2;
    const STANDING_CHARGE: f64 = 0.4;
    const CLIENT_ID: u64 = 0;

    fn init_log() {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_line_number(true)
            .with_target(false)
            .with_file(true)
            .init();
    }

    async fn create_client(db: Database) -> anyhow::Result<()> {
        let mut readings = vec![];
        readings.push(Reading::from(0.0));

        let token = bcrypt::hash(CLIENT_ID.to_string(), 4).expect("invalid bcrypt cost");
        let mut bills = vec![];
        let bill =
            Bill::from_reading(readings.last().unwrap(), UNIT_COST, STANDING_CHARGE).unwrap();
        bills.push(bill);

        let client = Client {
            token,
            bills,
            readings,
        };

        db.add_client(CLIENT_ID.to_string(), client).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_valid_auth() -> anyhow::Result<()> {
        init_log();

        let auth = Auth {
            id: CLIENT_ID,
            token: CLIENT_ID.to_string(),
        };

        let db = Arc::new(MockDatabase::new());
        create_client(db.clone()).await?;

        let framed = LengthDelimitedCodec::builder()
            .length_field_type::<u16>()
            .new_codec();
        let stream = CodecBuilder::new(framed)
            .read(&serde_json::to_vec(&auth).unwrap())
            .write(b"Authentication successful")
            .build();
        let store = Arc::new(RwLock::new(Store::new(2)));

        let _ = handle_client(stream, store, db).await;

        Ok(())
    }

    #[tokio::test]
    async fn test_invalid_token_auth() -> anyhow::Result<()> {
        let auth = Auth {
            id: CLIENT_ID,
            token: 1.to_string(),
        };

        let db = Arc::new(MockDatabase::new());
        create_client(db.clone()).await?;

        let framed = LengthDelimitedCodec::builder()
            .length_field_type::<u16>()
            .new_codec();
        let stream = CodecBuilder::new(framed)
            .read(&serde_json::to_vec(&auth).unwrap())
            .write(b"Authentication failed")
            .build();
        let store = Arc::new(RwLock::new(Store::new(2)));

        let _ = handle_client(stream, store, db).await;

        Ok(())
    }

    #[tokio::test]
    async fn test_invalid_id_auth() -> anyhow::Result<()> {
        let auth = Auth {
            id: 1,
            token: CLIENT_ID.to_string(),
        };

        let db = Arc::new(MockDatabase::new());
        create_client(db.clone()).await?;

        let framed = LengthDelimitedCodec::builder()
            .length_field_type::<u16>()
            .new_codec();
        let stream = CodecBuilder::new(framed)
            .read(&serde_json::to_vec(&auth).unwrap())
            .write(b"Authentication failed")
            .build();
        let store = Arc::new(RwLock::new(Store::new(2)));

        let _ = handle_client(stream, store, db).await;

        Ok(())
    }

    #[tokio::test]
    async fn test_send() -> anyhow::Result<()> {
        let reading = ClientMessage::MeterReading { reading: 100.0 };
        let bill = ServerMessage::Bill(
            Bill::from_reading(&Reading::from(100.0), UNIT_COST, STANDING_CHARGE).unwrap(),
        );

        let auth = Auth {
            id: CLIENT_ID,
            token: CLIENT_ID.to_string(),
        };

        let db = Arc::new(MockDatabase::new());
        create_client(db.clone()).await?;

        let framed = LengthDelimitedCodec::builder()
            .length_field_type::<u16>()
            .new_codec();
        let stream = CodecBuilder::new(framed)
            .read(&serde_json::to_vec(&auth).unwrap())
            .write(b"Authentication successful")
            .read(&serde_json::to_vec(&reading).unwrap())
            .write(&serde_json::to_vec(&bill).unwrap())
            .build();

        let store = Arc::new(RwLock::new(Store::new(2)));
        let _ = handle_client(stream, store, db).await;

        Ok(())
    }

    #[tokio::test]
    async fn test_alert() -> anyhow::Result<()> {
        let auth = Auth {
            id: CLIENT_ID,
            token: CLIENT_ID.to_string(),
        };

        let db = Arc::new(MockDatabase::new());
        create_client(db.clone()).await?;

        let framed = LengthDelimitedCodec::builder()
            .length_field_type::<u16>()
            .new_codec();
        let stream = CodecBuilder::new(framed)
            .read(&serde_json::to_vec(&auth).unwrap())
            .write(b"Authentication successful")
            .write(
                &serde_json::to_vec(&ServerMessage::PowerGridIssue {
                    error: "power grid error".to_string(),
                })
                .unwrap(),
            )
            .build();

        let store = Arc::new(RwLock::new(Store::new(2)));
        {
            let mut store = store.write().await;
            store.broadcast_err("power grid error");
        }

        let _ = handle_client(stream, store, db).await;

        Ok(())
    }
}
