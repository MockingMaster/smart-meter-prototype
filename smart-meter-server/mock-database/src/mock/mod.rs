use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::{Bill, Client, DatabaseError, DatabaseInterface, DbResult, Reading};

mod test;

pub struct MockDatabase {
    clients: Arc<RwLock<HashMap<String, Arc<Mutex<Client>>>>>,
}

impl MockDatabase {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl DatabaseInterface for MockDatabase {
    async fn add_client(&self, client_id: String, client: Client) -> DbResult<()> {
        let mut clients = self.clients.write().await;
        if clients.contains_key(&client_id) {
            return Err(DatabaseError::DataConflict);
        }
        clients.insert(client_id, Arc::new(Mutex::new(client)));
        Ok(())
    }

    async fn remove_client(&self, client_id: &str) -> DbResult<()> {
        let mut clients = self.clients.write().await;
        if clients.remove(client_id).is_none() {
            return Err(DatabaseError::ClientNotFound);
        }
        Ok(())
    }

    async fn add_reading(&self, client_id: &str, reading: Reading) -> DbResult<()> {
        let clients = self.clients.read().await;
        match clients.get(client_id) {
            Some(client_lock) => {
                let mut client = client_lock.lock().await;
                client.readings.push(reading);
                Ok(())
            }
            None => Err(DatabaseError::ClientNotFound),
        }
    }

    async fn add_bill(&self, client_id: &str, bill: Bill) -> DbResult<()> {
        let clients = self.clients.read().await;
        match clients.get(client_id) {
            Some(client_lock) => {
                let mut client = client_lock.lock().await;
                client.bills.push(bill.clone());
                Ok(())
            }
            None => Err(DatabaseError::ClientNotFound),
        }
    }

    async fn last_bill(&self, client_id: &str) -> DbResult<Option<Bill>> {
        let clients = self.clients.read().await;
        match clients.get(client_id) {
            Some(client_lock) => {
                let client = client_lock.lock().await;
                Ok(client.bills.last().cloned())
            }
            None => Err(DatabaseError::ClientNotFound),
        }
    }

    async fn last_reading(&self, client_id: &str) -> DbResult<Option<Reading>> {
        let clients = self.clients.read().await;
        match clients.get(client_id) {
            Some(client_lock) => {
                let client = client_lock.lock().await;
                Ok(client.readings.last().cloned())
            }
            None => Err(DatabaseError::ClientNotFound),
        }
    }

    async fn update_last_bill(&self, client_id: &str, bill: Bill) -> DbResult<()> {
        let clients = self.clients.read().await;
        match clients.get(client_id) {
            Some(client_lock) => {
                let mut client = client_lock.lock().await;
                if let Some(last_bill) = client.bills.last_mut() {
                    *last_bill = bill;
                    Ok(())
                } else {
                    Err(DatabaseError::BillNotFound)
                }
            }
            None => Err(DatabaseError::ClientNotFound),
        }
    }

    async fn client_exists(&self, client_id: &str) -> DbResult<Option<String>> {
        match self.clients.read().await.get(client_id) {
            Some(client) => Ok(Some(client.lock().await.token.clone())),
            None => Ok(None),
        }
    }
}
