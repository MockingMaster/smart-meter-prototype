use std::{env, sync::Arc, time::Duration};

use anyhow::{Context, Error};

use futures::{SinkExt, StreamExt};
use mock_database::{
    connection_context::ConnectionContext, mock::MockDatabase, Bill, Client, Database, Reading,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use store::{Message, PowerGridError, Store};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::broadcast::Receiver;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
    select,
    sync::RwLock,
    time,
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::{debug, error, info, trace, warn};

mod mock;

const UNIT_COST: f64 = 0.2;
const STANDING_CHARGE: f64 = 0.4;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_line_number(true)
        .with_target(false)
        .with_file(true)
        .init();

    let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = TcpListener::bind(&addr).await?;

    info!("listening on {addr}");
    let store = Arc::new(RwLock::new(Store::new(2)));

    let db = Arc::new(MockDatabase::new());
    let num_clients = env::var("NCLIENT")
        .unwrap_or(128.to_string())
        .parse::<usize>()?;
    create_clients(db.clone(), num_clients).await?;
    debug!("created clients");

    let copy = store.clone();
    tokio::spawn(async move {
        let _ = handle_power_grid_errors(copy).await;
    });

    loop {
        let (socket, _) = listener.accept().await?;
        let store = store.clone();
        let db = db.clone();
        tokio::spawn(async move {
            trace!("new client connected");
            if let Err(err) = handle_client(socket, store, db).await {
                error!("{err}");
            }
        });
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum ServerMessage {
    Bill(Bill),
    PowerGridIssue { error: String },
    PowerGridIssueResolved {},
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type")]
enum ClientMessage {
    MeterReading { reading: f64 },
}

async fn send_current_powergrid<T>(
    message: PowerGridError,
    framed: &mut Framed<T, LengthDelimitedCodec>,
    id: u64,
) -> anyhow::Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let future = framed.send(
        serde_json::to_vec(&ServerMessage::PowerGridIssue {
            error: message.error().clone(),
        })?
        .into(),
    );
    time::timeout(Duration::from_secs(5), future).await??;
    trace!("CID[{id}] sent current power grid error");
    Ok(())
}

async fn handle_client<T>(
    stream: T,
    store: Arc<RwLock<Store>>,
    database: Database,
) -> anyhow::Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let mut framed = LengthDelimitedCodec::builder()
        .length_field_type::<u16>()
        .new_framed(stream);

    let id = handle_client_auth(&mut framed, &database).await?;

    let (message, rx) = {
        let mut store = store.write().await;
        match store.subscribe(id) {
            Some((message, rx)) => (message, rx),
            None => {
                error!("CID[{id}] already connected");
                time::timeout(
                    Duration::from_secs(5),
                    framed.send("Another smart meter is already connected".into()),
                )
                .await??;
                return Ok(());
            }
        }
    };

    if let Some(message) = message {
        if let Err(err) = send_current_powergrid(message, &mut framed, id).await {
            let mut store = store.write().await;
            store.unsubscribe(id);
            return Err(err);
        }
    }

    let mut ctx =
        ConnectionContext::new(id.to_string(), UNIT_COST, STANDING_CHARGE, database).await?;
    info!("CID[{id}] authenticated client");
    if let Err(err) = handle_client_context(&mut framed, &mut ctx, rx, id).await {
        error!("CID[{id}] error handline connection context: {err}");
    }

    {
        let mut store = store.write().await;
        store.unsubscribe(id);
    }
    ctx.flush().await?;
    Ok(())
}

async fn handle_client_context<T>(
    framed: &mut Framed<T, LengthDelimitedCodec>,
    ctx: &mut ConnectionContext,
    mut rx: Receiver<Message>,
    id: u64,
) -> anyhow::Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let initial_cost = ctx.current_reading().reading;
    let mut ticker = time::interval(Duration::from_secs(120));
    ticker.reset();
    'outer: loop {
        select! {
            result = framed.next() => {
                let frame = match result {
                    Some(frame) => frame,
                    None => {
                        warn!("CID[{id}] smart meter disconnected");
                        break 'outer;
                    }
                };

                let message: ClientMessage = match frame {
                    Ok(bytes) => serde_json::from_slice(&bytes[..])?,
                    Err(e) => {
                        error!("CID[{id}] Error reading message: {:?}", e);
                        break 'outer;
                    }
                };

                trace!("CID[{id}]: message: {:?}", message);
                match message {
                    ClientMessage::MeterReading { mut reading } => {
                        reading += initial_cost;

                        ctx.add_reading(Reading::from(reading)).await?;
                        let future = framed.send(serde_json::to_vec(&ServerMessage::Bill(ctx.current_bill()))?.into());
                        time::timeout(Duration::from_secs(5), future).await??;
                    }
                }

                ticker.reset();
            },
            alert = rx.recv() => {
                match alert {
                    Ok(alert) => handle_alert(&alert, framed).await?,
                    Err(_) => break 'outer
                };
            },
            _ = ticker.tick() => {
                warn!("CID[{id}] no message from smart meter, disconnecting");
                break 'outer
            }
        }
    }

    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
struct Auth {
    id: u64,
    token: String,
}

async fn handle_client_auth<T>(
    framed: &mut Framed<T, LengthDelimitedCodec>,
    database: &Database,
) -> anyhow::Result<u64>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let auth = time::timeout(Duration::from_secs(10), framed.next())
        .await
        .context("client auth read timeout")?
        .ok_or_else(|| Error::msg("failed to read auth message"))?
        .context("auth message read failed")?;

    let auth: Auth = serde_json::from_slice(&auth[..]).context("decode json")?;

    match database.client_exists(&auth.id.to_string()).await? {
        Some(token) => {
            if !bcrypt::verify(auth.token, &token)? {
                time::timeout(
                    Duration::from_secs(5),
                    framed.send("Authentication failed".into()),
                )
                .await??;
                return Err(Error::msg("invalid client auth"));
            }

            time::timeout(
                Duration::from_secs(5),
                framed.send("Authentication successful".into()),
            )
            .await??;
            Ok(auth.id)
        }
        None => {
            time::timeout(
                Duration::from_secs(5),
                framed.send("Authentication failed".into()),
            )
            .await??;

            Err(Error::msg("invalid client auth"))
        }
    }
}

async fn handle_alert<T>(
    alert: &Message,
    framed: &mut Framed<T, LengthDelimitedCodec>,
) -> anyhow::Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    match alert {
        Message::PowerGridError(power_grid_error) => {
            let message = serde_json::to_vec(&ServerMessage::PowerGridIssue {
                error: power_grid_error.error().to_string(),
            })?;
            time::timeout(Duration::from_secs(5), framed.send(message.into()))
                .await
                .context("handle_alert timeout")??;
        }
        Message::PowerGridErrorResolved {} => {
            let message = serde_json::to_vec(&ServerMessage::PowerGridIssueResolved {})?;
            time::timeout(Duration::from_secs(5), framed.send(message.into()))
                .await
                .context("handle_alert timeout")??;
        }
    };

    Ok(())
}

async fn create_clients(db: Database, count: usize) -> anyhow::Result<()> {
    let mut rng = rand::thread_rng();
    for i in 0..count {
        let mut readings = vec![];
        for i in 0..10 {
            readings.push(Reading::from(rng.gen_range(i as f64..i as f64 + 1.0)))
        }

        let token = bcrypt::hash(i.to_string(), 4).expect("invalid bcrypt cost");
        let mut bills = vec![];
        let bill =
            Bill::from_reading(readings.last().unwrap(), UNIT_COST, STANDING_CHARGE).unwrap();
        bills.push(bill);

        let client = Client {
            token,
            bills,
            readings,
        };

        db.add_client(i.to_string(), client).await?;
    }

    Ok(())
}

async fn handle_power_grid_errors(store: Arc<RwLock<Store>>) -> anyhow::Result<()> {
    let mut stream = signal(SignalKind::user_defined1())?;

    loop {
        {
            stream.recv().await;
            let mut store = store.write().await;
            store.broadcast_err("someone unplugged the power cable!");
        }

        info!("sent error alert to connected clients");

        {
            stream.recv().await;
            let mut store = store.write().await;
            store.broadcast_resolved();
        }

        info!("sent issue resolved to connected clients");
    }
}

mod store {
    use std::collections::HashSet;

    use tokio::sync::broadcast;

    #[derive(Clone, PartialEq)]
    pub struct PowerGridError {
        error: String,
    }

    impl PowerGridError {
        pub fn error(&self) -> &String {
            &self.error
        }
    }

    #[derive(Clone, PartialEq)]
    pub enum Message {
        PowerGridError(PowerGridError),
        PowerGridErrorResolved {},
    }

    pub struct Store {
        tx: broadcast::Sender<Message>,
        alert: Option<PowerGridError>,
        clients: HashSet<u64>,
    }

    impl Store {
        pub fn new(capacity: usize) -> Store {
            let (tx, _) = broadcast::channel(capacity);

            Store {
                tx,
                alert: None,
                clients: HashSet::new(),
            }
        }

        pub fn subscribe(
            &mut self,
            cid: u64,
        ) -> Option<(Option<PowerGridError>, broadcast::Receiver<Message>)> {
            if !self.clients.insert(cid) {
                return None;
            }

            Some((self.alert.clone(), self.tx.subscribe()))
        }

        pub fn unsubscribe(&mut self, cid: u64) {
            self.clients.remove(&cid);
        }

        pub fn broadcast_err(&mut self, error: &str) {
            let error = PowerGridError {
                error: error.to_string(),
            };

            /*
             * if theres no one listening, send returns an error,
             * we don't really care about it though
             */
            let _ = self.tx.send(Message::PowerGridError(error.clone()));
            self.alert = Some(error);
        }

        pub fn broadcast_resolved(&mut self) {
            /*
             * if theres no one listening, send returns an error,
             * we don't really care about it though
             */
            let _ = self.tx.send(Message::PowerGridErrorResolved {});
            self.alert = None;
        }
    }
}
