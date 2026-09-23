use std::{fmt, future::Future};
use url::Url;
use zhorten_core::{
    ValidCode,
    api::{DashboardData, LinkRecord},
};

/// This trait defines the interface for the database operations that are used by the application.
pub trait Database {
    type Error: fmt::Display;

    fn dashboard(&self) -> Result<DashboardData, Self::Error>;

    fn create_link(
        &self,
        code: ValidCode,
        url: Url,
        created_at: i64,
    ) -> impl Future<Output = Result<Option<LinkRecord>, Self::Error>> + Send;

    fn remove_link(&self, code: &ValidCode)
    -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn follow_link(
        &self,
        code: &ValidCode,
        clicked_at: i64,
    ) -> impl Future<Output = Result<Option<String>, Self::Error>> + Send;
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

pub fn list_links<D: Database>(database: &D) -> Result<DashboardData, D::Error> {
    database.dashboard()
}

pub async fn create_link<D: Database>(
    database: &D,
    code: String,
    url: String,
    created_at: i64,
) -> Result<LinkRecord, CreateLinkError<D::Error>> {
    let code = ValidCode::try_from(code).map_err(|_| CreateLinkError::InvalidCode)?;
    let url = Url::parse(&url).map_err(|_| CreateLinkError::InvalidUrl)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(CreateLinkError::UnsupportedUrlScheme);
    }
    database
        .create_link(code, url, created_at)
        .await
        .map_err(CreateLinkError::Database)?
        .ok_or(CreateLinkError::Conflict)
}

pub async fn remove_link<D: Database>(
    database: &D,
    code: String,
) -> Result<(), RemoveLinkError<D::Error>> {
    let code = ValidCode::try_from(code).map_err(|_| RemoveLinkError::InvalidCode)?;
    database
        .remove_link(&code)
        .await
        .map_err(RemoveLinkError::Database)
}

pub async fn follow_link<D: Database>(
    database: &D,
    code: String,
    clicked_at: i64,
) -> Result<String, FollowLinkError<D::Error>> {
    let code = ValidCode::try_from(code).map_err(|_| FollowLinkError::InvalidCode)?;
    database
        .follow_link(&code, clicked_at)
        .await
        .map_err(FollowLinkError::Database)?
        .ok_or(FollowLinkError::NotFound)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
