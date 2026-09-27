use std::{fmt, sync::Arc};

use async_trait::async_trait;
use url::Url;
use zhorten_core::{
    ValidCode,
    api::{DashboardData, LinkRecord},
};

/// This trait defines the interface for the database operations that are used by the application.
#[async_trait]
pub trait Database: Send + Sync {
    type Error: fmt::Display + Send + Sync + 'static;

    async fn dashboard(&self) -> Result<DashboardData, Self::Error>;

    async fn create_link(
        &self,
        code: ValidCode,
        url: Url,
        created_at: i64,
    ) -> Result<Option<LinkRecord>, Self::Error>;

    async fn remove_link(&self, code: &ValidCode) -> Result<(), Self::Error>;

    async fn follow_link(
        &self,
        code: &ValidCode,
        context: ClickContext,
    ) -> Result<Option<String>, Self::Error>;
}

#[async_trait]
impl<D> Database for Arc<D>
where
    D: Database + ?Sized,
{
    type Error = D::Error;

    async fn dashboard(&self) -> Result<DashboardData, Self::Error> {
        self.as_ref().dashboard().await
    }

    async fn create_link(
        &self,
        code: ValidCode,
        url: Url,
        created_at: i64,
    ) -> Result<Option<LinkRecord>, Self::Error> {
        self.as_ref().create_link(code, url, created_at).await
    }

    async fn remove_link(&self, code: &ValidCode) -> Result<(), Self::Error> {
        self.as_ref().remove_link(code).await
    }

    async fn follow_link(
        &self,
        code: &ValidCode,
        context: ClickContext,
    ) -> Result<Option<String>, Self::Error> {
        self.as_ref().follow_link(code, context).await
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClickContext {
    pub clicked_at: i64,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
}

impl ClickContext {
    pub fn new(clicked_at: i64) -> Self {
        Self {
            clicked_at,
            client_ip: None,
            user_agent: None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CreateLinkError<E> {
    InvalidCode,
    InvalidUrl,
    UnsupportedUrlScheme,
    Conflict,
    Database(E),
}

#[derive(Debug, Eq, PartialEq)]
pub enum RemoveLinkError<E> {
    InvalidCode,
    Database(E),
}

#[derive(Debug, Eq, PartialEq)]
pub enum FollowLinkError<E> {
    InvalidCode,
    NotFound,
    Database(E),
}

pub async fn list_links<D: Database + ?Sized>(database: &D) -> Result<DashboardData, D::Error> {
    database.dashboard().await
}

pub async fn create_link<D: Database + ?Sized>(
    database: &D,
    code: String,
    url: impl AsRef<str>,
    created_at: i64,
) -> Result<LinkRecord, CreateLinkError<D::Error>> {
    let code = ValidCode::try_from(code).map_err(|_| CreateLinkError::InvalidCode)?;
    let url = Url::parse(url.as_ref()).map_err(|_| CreateLinkError::InvalidUrl)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(CreateLinkError::UnsupportedUrlScheme);
    }
    database
        .create_link(code, url, created_at)
        .await
        .map_err(CreateLinkError::Database)?
        .ok_or(CreateLinkError::Conflict)
}

pub async fn remove_link<D: Database + ?Sized>(
    database: &D,
    code: String,
) -> Result<(), RemoveLinkError<D::Error>> {
    let code = ValidCode::try_from(code).map_err(|_| RemoveLinkError::InvalidCode)?;
    database
        .remove_link(&code)
        .await
        .map_err(RemoveLinkError::Database)
}

pub async fn follow_link<D: Database + ?Sized>(
    database: &D,
    code: String,
    context: ClickContext,
) -> Result<String, FollowLinkError<D::Error>> {
    let code = ValidCode::try_from(code).map_err(|_| FollowLinkError::InvalidCode)?;
    database
        .follow_link(&code, context)
        .await
        .map_err(FollowLinkError::Database)?
        .ok_or(FollowLinkError::NotFound)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
