use anyhow::Result;
use tonic::transport::Channel;

pub mod db {
    #![allow(clippy::all, clippy::pedantic)]
    tonic::include_proto!("db");
}

#[derive(Clone)]
pub struct DbClient {
    inner: db::database_client::DatabaseClient<Channel>,
}

impl DbClient {
    /// Connect to the database service
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    pub async fn connect(endpoint: String) -> Result<Self> {
        let channel = Channel::from_shared(endpoint)?.connect().await?;
        Ok(Self {
            inner: db::database_client::DatabaseClient::new(channel),
        })
    }

    /// Put a value into the database
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn put(&mut self, collection: &str, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        let req = db::PutRequest {
            collection: collection.to_string(),
            key,
            value,
        };
        let resp = self.inner.put(req).await?;
        if resp.get_ref().success {
            Ok(())
        } else {
            anyhow::bail!(resp.get_ref().error.clone())
        }
    }

    /// Get a value from the database
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get(&mut self, collection: &str, key: Vec<u8>) -> Result<Option<Vec<u8>>> {
        let req = db::GetRequest {
            collection: collection.to_string(),
            key,
        };
        let resp = self.inner.get(req).await?;
        let r = resp.into_inner();
        if r.found { Ok(Some(r.value)) } else { Ok(None) }
    }
}
