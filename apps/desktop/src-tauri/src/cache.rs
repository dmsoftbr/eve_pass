//! Local encrypted cache (SQLite via rusqlite). It stores **envelopes** — which
//! are already AEAD ciphertext bound to the vault key — so item content is
//! protected at rest even without SQLCipher (that is a Fase 2 defense-in-depth
//! add-on). Plus a dirty flag driving the offline upload queue.

use std::path::Path;

use rusqlite::{params, Connection};

pub struct Cache {
    conn: Connection,
}

#[derive(Clone)]
pub struct CacheRow {
    pub id: String,
    pub envelope: Vec<u8>,
    pub revision: i64,
    pub updated_at: String,
    pub deleted: bool,
    pub dirty: bool,
    /// Set for shared items: decrypt with this collection's key, not the vault key.
    pub collection_id: Option<String>,
}

/// Which table a row lives in. Items and folders share the same shape.
#[derive(Clone, Copy)]
pub enum Kind {
    Item,
    Folder,
}

impl Kind {
    fn table(self) -> &'static str {
        match self {
            Kind::Item => "cache_items",
            Kind::Folder => "cache_folders",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Item => "item",
            Kind::Folder => "folder",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "item" => Some(Kind::Item),
            "folder" => Some(Kind::Folder),
            _ => None,
        }
    }
}

impl Cache {
    pub fn open(path: &Path) -> rusqlite::Result<Cache> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            r#"
            create table if not exists cache_items (
              id text primary key,
              envelope blob not null,
              revision integer not null,
              updated_at text,
              deleted integer default 0,
              dirty integer default 0,
              collection_id text
            );
            create table if not exists cache_folders (
              id text primary key,
              envelope blob not null,
              revision integer not null,
              updated_at text,
              deleted integer default 0,
              dirty integer default 0,
              collection_id text
            );
            create table if not exists meta (key text primary key, value text);
            "#,
        )?;
        Ok(Cache { conn })
    }

    /// Insert or replace a row.
    pub fn upsert(&self, kind: Kind, row: &CacheRow) -> rusqlite::Result<()> {
        let sql = format!(
            "insert into {t} (id, envelope, revision, updated_at, deleted, dirty, collection_id)
             values (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             on conflict(id) do update set
               envelope=excluded.envelope, revision=excluded.revision,
               updated_at=excluded.updated_at, deleted=excluded.deleted, dirty=excluded.dirty,
               collection_id=excluded.collection_id",
            t = kind.table()
        );
        self.conn.execute(
            &sql,
            params![
                row.id,
                row.envelope,
                row.revision,
                row.updated_at,
                row.deleted as i64,
                row.dirty as i64,
                row.collection_id
            ],
        )?;
        Ok(())
    }

    /// Remove all cached items belonging to a collection (used when the
    /// collection is deleted / access is lost, so they don't linger undecryptable).
    pub fn delete_collection_items(&self, collection_id: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("delete from cache_items where collection_id=?1", params![collection_id])?;
        Ok(())
    }

    pub fn get(&self, kind: Kind, id: &str) -> rusqlite::Result<Option<CacheRow>> {
        let sql = format!(
            "select id, envelope, revision, updated_at, deleted, dirty, collection_id from {} where id=?1",
            kind.table()
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![id])?;
        if let Some(r) = rows.next()? {
            Ok(Some(row_from(r)?))
        } else {
            Ok(None)
        }
    }

    /// All non-deleted rows.
    pub fn list(&self, kind: Kind) -> rusqlite::Result<Vec<CacheRow>> {
        let sql = format!(
            "select id, envelope, revision, updated_at, deleted, dirty, collection_id from {} where deleted=0
             order by updated_at desc",
            kind.table()
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| row_from(r))?;
        rows.collect()
    }

    /// Rows pending upload (dirty=1), regardless of deleted state.
    pub fn pending(&self, kind: Kind) -> rusqlite::Result<Vec<CacheRow>> {
        let sql = format!(
            "select id, envelope, revision, updated_at, deleted, dirty, collection_id from {} where dirty=1",
            kind.table()
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| row_from(r))?;
        rows.collect()
    }

    /// Clear the dirty flag once the JS side confirms the upload, but only if
    /// the confirmed revision still matches (avoids clobbering a newer edit).
    pub fn mark_synced(&self, kind: Kind, id: &str, revision: i64) -> rusqlite::Result<()> {
        let sql = format!("update {} set dirty=0 where id=?1 and revision=?2", kind.table());
        self.conn.execute(&sql, params![id, revision])?;
        Ok(())
    }
}

fn row_from(r: &rusqlite::Row) -> rusqlite::Result<CacheRow> {
    Ok(CacheRow {
        id: r.get(0)?,
        envelope: r.get(1)?,
        revision: r.get(2)?,
        updated_at: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
        deleted: r.get::<_, i64>(4)? != 0,
        dirty: r.get::<_, i64>(5)? != 0,
        collection_id: r.get::<_, Option<String>>(6)?,
    })
}
