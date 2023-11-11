use crate::{
    error::{Error, ErrorKind, Result},
    utils::{self, ListCompareResult},
};
use sqlx::{
    migrate::MigrateDatabase,
    sqlite::{SqliteConnectOptions, SqliteRow},
    ConnectOptions, Connection, Row, Sqlite, SqliteConnection,
};
use std::{fs, path::Path, str::FromStr};

pub struct DB {
    connection: SqliteConnection,
}

pub struct Item {
    pub hash: String,
    pub title: String,
    pub ext: String,
    pub collection_id: i64,
    pub tags: Vec<String>,
}

impl sqlx::FromRow<'_, SqliteRow> for Item {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Item {
            hash: row.try_get("hash")?,
            title: row.try_get("title")?,
            ext: row.try_get("ext")?,
            collection_id: row.try_get("collection_id")?,
            tags: Vec::new(),
        })
    }
}

impl DB {
    /// Create or connect to a vorg db.
    ///
    /// If the db does not exist, this creates a new vorg db.
    /// If the db does exist, this connects to the db.
    ///
    /// # Errors
    /// - `ErrorKind::DB` when encountered database error either when creating a new database or
    ///   opening/validating an existing one, e.g. invalid database or table structure.
    /// - `ErrorKind::IO` when encountered IO error creating the parent folder of `db_path`, if it
    ///   does not exist.
    pub async fn new<T>(db_path: T) -> Result<Self>
    where
        T: AsRef<Path>,
    {
        // Convert db_path to str
        let db_path = db_path.as_ref();
        let db_path_string = db_path.to_string_lossy().into_owned();

        // Check for db existence
        if Sqlite::database_exists(&db_path_string).await? {
            // Database exists
            let mut connection = SqliteConnectOptions::from_str(&db_path_string)?
                .connect()
                .await?;
            DB::validate_db(&mut connection)
                .await
                .map(|_| DB { connection })
        } else {
            // Database does not exist, create a new one
            let db_path_parent = db_path
                .parent()
                .expect("Database's path should have a parent, i.e. not root.");
            fs::create_dir_all(db_path_parent)?;
            DB::create_db(&db_path_string)
                .await
                .map(|connection| DB { connection })
        }
    }

    /// Creates a new sqlite db to be used as vorg db.
    ///
    /// This function assumes the database does not exist. This is enforced by create_repo which
    /// ensures the repo folder is empty before calling this function.
    /// This function also requires that the parent of `db_path_str` exists and is a folder.
    async fn create_db(db_path_str: &str) -> Result<SqliteConnection> {
        // Create database and connect to it
        Sqlite::create_database(db_path_str).await?;
        let mut connection = SqliteConnection::connect(db_path_str).await?;

        // Initialize tables
        sqlx::query(
        "
            CREATE TABLE tags (
                tag_id INTEGER PRIMARY KEY NOT NULL,
                name TEXT NOT NULL
            );
            CREATE TABLE collections (
                collection_id INTEGER PRIMARY KEY NOT NULL,
                title TEXT NOT NULL
            );
            CREATE TABLE items (
                item_id INTEGER PRIMARY KEY NOT NULL,
                collection_id INTEGER NOT NULL,
                ext TEXT NOT NULL,
                hash VARCHAR(64) NOT NULL,
                FOREIGN KEY (collection_id) REFERENCES collections(collection_id)
            );
            CREATE TABLE collection_tag (
                collection_id INTEGER NOT NULL,
                tag_id INTEGER NOT NULL,
                PRIMARY KEY (collection_id, tag_id),
                FOREIGN KEY (collection_id) REFERENCES collections(collection_id),
                FOREIGN KEY (tag_id) REFERENCES tags(tag_id)
            );
            CREATE VIRTUAL TABLE title_fts USING fts5(
                title,
                content='collections',
                content_rowid='collection_id'
            );
            CREATE TRIGGER title_insert AFTER INSERT ON collections BEGIN
                INSERT INTO title_fts(rowid, title) VALUES (new.collection_id, new.title);
            END;
            CREATE TRIGGER title_delete AFTER DELETE ON collections BEGIN
                INSERT INTO title_fts(title_fts, rowid, title)
                    VALUES('delete', old.collection_id, old.title);
            END;
            CREATE TRIGGER title_update AFTER UPDATE ON collections BEGIN
                INSERT INTO title_fts(fts_idx, rowid, title) VALUES('delete', old.collection_id, old.title);
                INSERT INTO title_fts(rowid, title) VALUES (new.collection_id, new.title);
            END;
            CREATE UNIQUE INDEX hash_index ON items (hash);
            CREATE UNIQUE INDEX tag_index ON tags (name);
            "
        ).execute(&mut connection).await?;

        Ok(connection)
    }

    /// Validates the strcture of a vorg db.
    ///
    /// If valid, returns no error.
    /// If not valid, returns a `InvalidDatabase` error with a message describing why.
    async fn validate_db(connection: &mut SqliteConnection) -> Result<()> {
        static EXPECTED_TABLE_NAMES: [&str; 9] = [
            "collection_tag",
            "collections",
            "items",
            "tags",
            "title_fts",
            "title_fts_config",
            "title_fts_data",
            "title_fts_docsize",
            "title_fts_idx",
        ];
        static EXPECTED_INDICES: [&str; 2] = ["hash_index", "tag_index"];
        static EXPECTED_TRIGGERS: [&str; 3] = ["title_delete", "title_insert", "title_update"];
        static VERIFY_COLUMNS: [bool; 9] =
            [true, true, true, true, false, false, false, false, false];
        static EXPECTED_COLUMNS: [(usize, [(&str, &str); 4]); 4] = [
            // collection_tag
            (
                2,
                [
                    ("collection_id", "INTEGER"),
                    ("tag_id", "INTEGER"),
                    ("", ""),
                    ("", ""),
                ],
            ),
            // collections
            (
                2,
                [
                    ("collection_id", "INTEGER"),
                    ("title", "TEXT"),
                    ("", ""),
                    ("", ""),
                ],
            ),
            // items
            (
                4,
                [
                    ("collection_id", "INTEGER"),
                    ("ext", "TEXT"),
                    ("hash", "VARCHAR(64)"),
                    ("item_id", "INTEGER"),
                ],
            ),
            // tags
            (
                2,
                [("name", "TEXT"), ("tag_id", "INTEGER"), ("", ""), ("", "")],
            ),
        ];

        let result = sqlx::query!(
            "
            SELECT tbl_name from sqlite_master
            WHERE type='table' ORDER BY tbl_name
            "
        )
        .map(|row| row.tbl_name)
        .fetch_all(&mut *connection)
        .await?;
        let table_names: Vec<&str> = result
            .iter()
            .filter_map(|tbl_name_option| {
                tbl_name_option
                    .as_ref()
                    .and_then(|tbl_name| Some(tbl_name.as_str()))
            })
            .collect();

        // Validate table name
        let compare_result = utils::compare_lists(
            &table_names,
            &EXPECTED_TABLE_NAMES,
            |table_name| table_name,
            |_, _| true,
        );
        match compare_result {
            ListCompareResult::Missing(table_name) => {
                return Err(Error {
                    msg: format!("Table \"{table_name}\" is missing from the database.",),
                    kind: ErrorKind::DB,
                });
            }
            ListCompareResult::Unexpected(table_name) => {
                return Err(Error {
                    msg: format!("Unexpected table \"{table_name}\" exists in the database."),
                    kind: ErrorKind::DB,
                });
            }
            ListCompareResult::Unequal(_) => {
                panic!("Unexpected compare result for table names!");
            }
            ListCompareResult::Identical => (),
        }

        // Validate table structure
        let mut columns_index = 0;
        for (index, table) in EXPECTED_TABLE_NAMES.iter().enumerate() {
            if VERIFY_COLUMNS[index] {
                DB::validate_table(
                    connection,
                    table,
                    &EXPECTED_COLUMNS[columns_index].1,
                    EXPECTED_COLUMNS[columns_index].0,
                )
                .await?;
                columns_index += 1;
            }
        }

        // Validate indices
        let result = sqlx::query!(
            "
            SELECT name FROM sqlite_master
            WHERE type = 'index'
            AND sql IS NOT NULL
            ORDER BY name
            "
        )
        .map(|row| row.name)
        .fetch_all(&mut *connection)
        .await?;
        let indices: Vec<&str> = result
            .iter()
            .filter_map(|index_name| index_name.as_ref().and_then(|name| Some(name.as_str())))
            .collect();
        let compare_result = utils::compare_lists(
            &indices,
            &EXPECTED_INDICES,
            |index_name| index_name,
            |_, _| true,
        );
        match compare_result {
            ListCompareResult::Identical => (),
            _ => {
                return Err(Error {
                    msg: format!("Database has unexpected or missing indices."),
                    kind: ErrorKind::DB,
                });
            }
        }

        // Validate triggers
        let result = sqlx::query!(
            "
            SELECT name FROM sqlite_master
            WHERE type = 'trigger'
            ORDER BY name
            "
        )
        .map(|row| row.name)
        .fetch_all(&mut *connection)
        .await?;
        let triggers: Vec<&str> = result
            .iter()
            .filter_map(|index_name| index_name.as_ref().and_then(|name| Some(name.as_str())))
            .collect();
        let compare_result = utils::compare_lists(
            &triggers,
            &EXPECTED_TRIGGERS,
            |index_name| index_name,
            |_, _| true,
        );
        match compare_result {
            ListCompareResult::Identical => (),
            _ => {
                return Err(Error {
                    msg: format!("Database has unexpected or missing triggers."),
                    kind: ErrorKind::DB,
                });
            }
        }

        Ok(())
    }

    /// Validates the strcture of a vorg db table.
    ///
    /// If valid, returns no error.
    /// If not valid, returns a `InvalidDatabase` error with a message describing why.
    async fn validate_table(
        connection: &mut SqliteConnection,
        table_name: &str,
        expected_columns: &[(&str, &str)],
        expected_column_count: usize,
    ) -> Result<()> {
        let columns: Vec<(String, String)> =
            sqlx::query("SELECT name,type FROM pragma_table_info(?) ORDER BY name")
                .bind(table_name)
                .try_map(|row: SqliteRow| Ok((row.try_get("name")?, row.try_get("type")?)))
                .fetch_all(connection)
                .await?;

        let columns: Vec<(&str, &str)> = columns
            .iter()
            .map(|column| (column.0.as_str(), column.1.as_str()))
            .collect();

        // Compare columns
        let compare_result = utils::compare_lists(
            &columns,
            &expected_columns[..expected_column_count],
            |column| &column.0,
            |column_1, column_2| column_1.1 == column_2.1,
        );
        match compare_result {
            ListCompareResult::Missing(column) => {
                return Err(Error {
                    msg: format!(
                        "Column \"{}\" is missing from table \"{table_name}\".",
                        column.0
                    ),
                    kind: ErrorKind::DB,
                });
            }
            ListCompareResult::Unexpected(column) => {
                return Err(Error {
                    msg: format!(
                        "Unexpected column \"{}\" in table \"{table_name}\".",
                        column.0
                    ),
                    kind: ErrorKind::DB,
                });
            }
            ListCompareResult::Unequal(column) => {
                return Err(Error {
                    msg: format!(
                        "Column \"{}\" in table \"{table_name}\" should have type \"{}\".",
                        column.0, column.1
                    ),
                    kind: ErrorKind::DB,
                });
            }
            ListCompareResult::Identical => (),
        }

        Ok(())
    }

    /// Start a new SQL transaction
    async fn begin_transaction(&mut self) -> Result<()> {
        sqlx::query!("BEGIN TRANSACTION")
            .execute(&mut self.connection)
            .await?;
        Ok(())
    }

    /// Commit SQL transaction
    async fn commit_transaction(&mut self) -> Result<()> {
        sqlx::query!("COMMIT TRANSACTION")
            .execute(&mut self.connection)
            .await?;
        Ok(())
    }

    /// Add a new collection in db
    async fn add_collection(&mut self, title: &str) -> Result<i64> {
        let collection_id = sqlx::query!(
            "
            INSERT INTO collections(title) VALUES(?)
            RETURNING collection_id;
            ",
            title
        )
        .map(|row| row.collection_id)
        .fetch_one(&mut self.connection)
        .await?;
        Ok(collection_id)
    }

    async fn add_item_to_collection(
        &mut self,
        collection_id: i64,
        hash: &str,
        ext: &str,
    ) -> Result<i64> {
        let item_id = sqlx::query!(
            "
            INSERT OR ROLLBACK INTO items(collection_id, hash, ext)
            VALUES (?, ?, ?)
            RETURNING item_id
            ",
            collection_id,
            hash,
            ext
        )
        .map(|row| row.item_id)
        .fetch_one(&mut self.connection)
        .await?;
        Ok(item_id)
    }

    /// Insert a new tag for an item.
    pub async fn add_tag_to_collection(&mut self, collection_id: i64, tag: &str) -> Result<()> {
        // Check if the given $name exists
        sqlx::query!("INSERT OR IGNORE INTO tags(name) VALUES (?)", tag)
            .execute(&mut self.connection)
            .await?;
        sqlx::query!(
            "
            INSERT INTO collection_tag(collection_id, tag_id)
            SELECT ?, tag_id FROM tags WHERE name=?;
            ",
            collection_id,
            tag
        )
        .execute(&mut self.connection)
        .await?;
        Ok(())
    }

    /// Import a file into the database with an Incomplete tag.
    pub async fn import_file(&mut self, title: &str, hash: &str, ext: &str) -> Result<()> {
        self.begin_transaction().await?;
        // Add collection
        let collection_id = self.add_collection(title).await?;
        // Add item to collection
        let Ok(item_id) = self.add_item_to_collection(collection_id, hash, ext).await else {
            return Err(Error {
                msg: String::from("The item to import already exists in the database."),
                kind: ErrorKind::Duplicate,
            });
        };
        // Add tag
        self.add_tag_to_collection(item_id, "meta:Incomplete")
            .await?;
        self.commit_transaction().await?;
        Ok(())
    }

    /// Get files that satisfy the given filter.
    ///
    /// TODO: Add filtering.
    pub async fn get_items(&mut self) -> Result<Vec<Item>> {
        // Access items table
        let items_query = "
        SELECT hash, title, ext, c.collection_id
        FROM collections c
        JOIN items i ON c.collection_id = i.collection_id
        ORDER BY hash
        ";
        let mut items = sqlx::query_as::<_, Item>(items_query)
            .fetch_all(&mut self.connection)
            .await?;

        for item in items.iter_mut() {
            let tags = sqlx::query!(
                "
                SELECT name FROM tags t
                JOIN collection_tag ct
                ON ct.tag_id = t.tag_id
                JOIN collections c
                ON c.collection_id = ct.collection_id
                WHERE c.collection_id = ?
                ",
                item.collection_id
            )
            .map(|row| row.name)
            .fetch_all(&mut self.connection)
            .await?;
            item.tags = tags;
        }

        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use test_context::{test_context, AsyncTestContext};
    use tokio::time::{sleep, Duration};
    use uuid::Uuid;

    struct TempFolder {
        pub path: std::path::PathBuf,
    }

    #[async_trait::async_trait]
    impl AsyncTestContext for TempFolder {
        async fn setup() -> TempFolder {
            let uuid = Uuid::new_v4();
            let temp_dir_path =
                String::from("temp-") + uuid.hyphenated().encode_lower(&mut Uuid::encode_buffer());
            let temp_dir = std::path::PathBuf::from(temp_dir_path);
            fs::create_dir(&temp_dir).expect("Failed to create temp dir for testing.");
            TempFolder { path: temp_dir }
        }

        async fn teardown(self) {
            if let Err(_) = fs::remove_dir_all(&self.path) {
                // If the first try failed, wait a bit and retry
                sleep(Duration::from_millis(200)).await;
                fs::remove_dir_all(&self.path).expect("Failed to teardown temp test directory.")
            };
        }
    }

    #[test_context(TempFolder)]
    #[tokio::test]
    async fn test_create_db_success(ctx: &TempFolder) -> Result<()> {
        // GIVEN
        let db_path = ctx.path.join("vorg.db");

        // WHEN
        DB::new(&db_path).await?;

        // THEN
        // Verify a connection can be opened
        let mut db = SqliteConnection::connect(&db_path.to_string_lossy()).await?;

        // Verify required tables
        let num_rows = sqlx::query!(
            "
            SELECT tbl_name FROM sqlite_master
            WHERE type='table'
            AND tbl_name IN (
                'tags', 'items', 'collections', 'collection_tag', 'title_fts'
            );
            ",
        )
        .fetch_all(&mut db)
        .await?
        .len();
        assert_eq!(num_rows, 5);

        // Verify required indices
        let num_rows = sqlx::query!(
            "
            SELECT tbl_name FROM sqlite_master
            WHERE type='index'
            AND name IN (
                'hash_index', 'tag_index'
            );
            ",
        )
        .fetch_all(&mut db)
        .await?
        .len();
        assert_eq!(num_rows, 2);

        Ok(())
    }

    #[test_context(TempFolder)]
    #[tokio::test]
    async fn test_create_db_failed_db(ctx: &TempFolder) -> Result<()> {
        // GIVEN
        let db_path = ctx.path.join("vorg.db");

        // Create a folder at the target db
        fs::create_dir_all(&db_path)?;

        // WHEN
        let result = DB::new(&db_path).await;

        // THEN
        assert!(result.is_err());
        if let Err(error) = result {
            assert!(matches!(error.kind, ErrorKind::DB));
        }

        Ok(())
    }

    #[test_context(TempFolder)]
    #[tokio::test]
    async fn test_create_db_failed_io(ctx: &TempFolder) -> Result<()> {
        // GIVEN
        let db_path = ctx.path.join("parent").join("vorg.db");

        // Create a file at the parent path
        let parent_path = db_path.parent().ok_or(Error {
            kind: ErrorKind::IO,
            msg: String::from("Failed to get db path parent."),
        })?;
        fs::File::create(parent_path)?;

        // WHEN
        let result = DB::new(&db_path).await;

        // THEN
        assert!(result.is_err());
        if let Err(error) = result {
            assert!(matches!(error.kind, ErrorKind::IO));
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_open_db_success() -> Result<()> {
        DB::new("resources/db/valid.db").await?;

        Ok(())
    }

    #[rstest]
    #[case(
        "resources/db/invalid_unexpected_table.db",
        "Unexpected table \"table_unexpected\" exists in the database."
    )]
    #[case(
        "resources/db/invalid_missing_table.db",
        "Table \"items\" is missing from the database."
    )]
    #[case(
        "resources/db/invalid_unexpected_column.db",
        "Unexpected column \"studio_id\" in table \"items\"."
    )]
    #[case(
        "resources/db/invalid_missing_column.db",
        "Column \"ext\" is missing from table \"items\"."
    )]
    #[case(
        "resources/db/invalid_wrong_column_type.db",
        "Column \"hash\" in table \"items\" should have type \"VARCHAR(64)\"."
    )]
    #[case(
        "resources/db/invalid_missing_index.db",
        "Database has unexpected or missing indices."
    )]
    #[case(
        "resources/db/invalid_missing_trigger.db",
        "Database has unexpected or missing triggers."
    )]
    #[tokio::test]
    async fn test_open_db_error(#[case] db_path: &str, #[case] err_msg: &str) {
        // WHEN
        let result = DB::new(db_path).await;

        // THEN
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error.kind, ErrorKind::DB);
            assert_eq!(error.to_string(), err_msg);
        }
    }

    #[test_context(TempFolder)]
    #[tokio::test]
    async fn test_import_file(ctx: &TempFolder) -> Result<()> {
        // GIVEN
        let db_path = ctx.path.join("vorg.db");
        let mut db = DB::new(&db_path).await.unwrap();

        // WHEN
        // Import file
        let title = "Test title";
        let ext = "mp4";
        let hash = "09c683231bb0e88e84a8408fdbfe174c70d83d03e0604eb612631e79";
        let result = db.import_file(&title, &hash, &ext).await;

        // THEN
        assert!(result.is_ok());
        // Test file has been imported
        let mut connection = SqliteConnection::connect(&db_path.to_string_lossy()).await?;
        let item_exists_query = "
        SELECT hash FROM collections c, items i, collection_tag ct, tags t
        WHERE c.collection_id=ct.collection_id
        AND ct.tag_id=t.tag_id
        AND i.collection_id=c.collection_id
        AND t.name='meta:Incomplete'
        AND title=?
        AND ext=?
        AND hash=?
        ";
        assert_eq!(
            sqlx::query(item_exists_query)
                .bind(title)
                .bind(ext)
                .bind(hash)
                .fetch_all(&mut connection)
                .await?
                .len(),
            1
        );

        // WHEN
        // Test duplicate import
        let duplicate_title = "Another title";
        let result = db.import_file(duplicate_title, &hash, &ext).await;

        // THEN
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "The item to import already exists in the database."
        );
        // Make sure no redundant collection is created.
        assert_eq!(
            sqlx::query!(
                "
                SELECT title FROM collections
                WHERE title = ?
                ",
                duplicate_title
            )
            .fetch_all(&mut connection)
            .await?
            .len(),
            0
        );

        // WHEN
        // Test reusing tag
        let hash2 = "4effadeed3957d9dab1a645b9a7d01c18380d54e71d51148fdf84633";
        let title2 = "Some title";
        let result = db.import_file(&title2, &hash2, &ext).await;

        // THEN
        assert!(result.is_ok());
        assert_eq!(
            sqlx::query(item_exists_query)
                .bind(title2)
                .bind(ext)
                .bind(hash2)
                .fetch_all(&mut connection)
                .await?
                .len(),
            1
        );

        Ok(())
    }

    #[test_context(TempFolder)]
    #[tokio::test]
    async fn test_get_items(ctx: &TempFolder) -> Result<()> {
        // GIVEN
        let db_path = ctx.path.join("vorg.db");
        let mut db = DB::new(&db_path).await.unwrap();

        // Import file
        let title = "Test title";
        let ext = "mp4";
        let hash = "09c683231bb0e88e84a8408fdbfe174c70d83d03e0604eb612631e79";
        let result = db.import_file(&title, &hash, &ext).await;
        assert!(result.is_ok());

        // WHEN
        let items = db.get_items().await?;

        // THEN
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, title);
        assert_eq!(items[0].ext, ext);
        assert_eq!(items[0].hash, hash);
        assert_eq!(items[0].tags.len(), 1);
        assert_eq!(items[0].tags[0], "meta:Incomplete");
        Ok(())
    }
}
