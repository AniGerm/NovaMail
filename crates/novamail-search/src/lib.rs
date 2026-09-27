//! Full-text search over the local SQLite FTS5 index.

use novamail_db::Database;
use novamail_ipc::{SearchRequest, SearchResponse};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SearchError {
    #[error(transparent)]
    Db(#[from] novamail_db::DbError),
}

pub type SearchResult<T> = Result<T, SearchError>;

pub struct SearchService {
    db: Database,
}

impl SearchService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn search(&self, request: SearchRequest) -> SearchResult<SearchResponse> {
        let (messages, total) = self.db.list_messages(&novamail_ipc::ListMessagesRequest {
            mailbox_id: None,
            account_id: request.account_id,
            unified: false,
            mailbox_role: None,
            local_only: false,
            limit: request.limit,
            offset: request.offset,
            query: Some(request.query),
            unread_only: false,
            starred_only: false,
            has_attachments: false,
            sort_by: Default::default(),
            sort_dir: Default::default(),
        })?;
        Ok(SearchResponse { messages, total })
    }
}
