use crate::{
    error::{Error, ErrorKind, Result},
    utils::{self, ListCompareResult},
};
use sqlx::{
    migrate::MigrateDatabase,
    sqlite::{SqliteConnectOptions, SqliteRow},
    ConnectOptions, Connection, Row, Sqlite, SqliteConnection,
};
use std::{fmt::Debug, fs, path::Path, str::FromStr};

pub struct DB {
    connection: SqliteConnection,
}

// Placeholder implementation for unwrap_err
impl Debug for DB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Placeholder debug implementation for vorgrs::db::DB")
    }
}

pub struct File {
    pub hash: String,
    pub title: String,
    pub ext: String,
    pub studio: Option<String>,
    pub actors: Vec<String>,
    pub tags: Vec<String>,
}

impl sqlx::FromRow<'_, SqliteRow> for File {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(File {
            hash: row.try_get("hash")?,
            title: row.try_get("title")?,
            ext: row.try_get("ext")?,
            studio: row.try_get("studio_name")?,
            actors: Vec::new(),
            tags: Vec::new(),
        })
    }
}

/// Macro definitions for defining management routines (like insert and cleanup) for many-to-many
/// metadata (like actors and tags).
macro_rules! define_insert_routine {
    ( $name:ident, $insert_name:ident ) => {
        /**
         * Insert a new $name for an item.
         */
        pub async fn $insert_name(&mut self, item_id: i64, $name: &str) -> Result<()> {
            // Check if the given $name exists
            let query = concat!(
                "INSERT OR IGNORE INTO ",
                stringify!($name),
                "s(name) VALUES (?)"
            );
            sqlx::query(query)
                .bind($name)
                .execute(&mut self.connection)
                .await?;
            let query = concat!(
                "INSERT INTO item_",
                stringify!($name),
                "(item_id, ",
                stringify!($name),
                "_id) SELECT ?, tag_id FROM tags",
                "WHERE ",
                stringify!($name),
                "_name=?"
            );
            sqlx::query(query)
                .bind(item_id)
                .bind($name)
                .execute(&mut self.connection)
                .await?;
            Ok(())
        }
    };
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
                .expect("db_path's parent should be a folder.");
            fs::create_dir_all(db_path_parent)?;
            DB::create_db(&db_path_string)
                .await
                .map(|connection| DB { connection })
        }
    }

    /// Import a file into the database with an Incomplete tag.
    pub async fn import_file(&mut self, title: &String, ext: &String, hash: &String) -> Result<()> {
        // TODO: return error on duplicates
        // Insert item
        let query = "
        INSERT INTO items (hash,title,studio_id,ext)
        VALUES (?, ?, 0, ?)
        RETURNING item_id
        ";
        let result_row = sqlx::query(query)
            .bind(hash)
            .bind(title)
            .bind(ext)
            .fetch_one(&mut self.connection)
            .await?;
        let item_id = result_row.try_get("item_id")?;
        // Add tag
        self.insert_tag(item_id, "Incomplete").await?;
        Ok(())
    }

    define_insert_routine!(tag, insert_tag);

    /// Get files that satisfy the given filter.
    ///
    /// TODO: Add filtering.
    /// TODO: Return tags and actors
    pub async fn get_files(&mut self) -> Result<Vec<File>> {
        // Access items table
        let query = "
        SELECT hash,title,ext,studio_id,name AS studio_name
        FROM items AS i
        JOIN studios AS s ON i.studio_id = s.studio_id
        ORDER BY hash
        ";
        let result = sqlx::query_as::<_, File>(query)
            .fetch_all(&mut self.connection)
            .await?;

        // TODO: get tags and actors

        Ok(result)
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
        let init_query = "
        CREATE TABLE tags (
            tag_id INTEGER PRIMARY KEY NOT NULL,
            name TEXT NOT NULL
        );
        CREATE TABLE collections (
            collection_id INTEGER PRIMARY KEY NOT NULL,
            title_id INTEGER NOT NULL,
            FOREIGN KEY (title_id) REFERENCES titles(rowid)
        );
        CREATE TABLE items (
            item_id INTEGER PRIMARY KEY NOT NULL,
            hash VARCHAR(64) NOT NULL,
            ext TEXT NOT NULL
        );
        CREATE TABLE collection_item (
            collection_id INTEGER NOT NULL,
            item_id INTEGER NOT NULL,
            PRIMARY KEY (collection_id, item_id),
            FOREIGN KEY (collection_id) REFERENCES collections(collection_id),
            FOREIGN KEY (item_id) REFERENCES items(item_id)
        );
        CREATE TABLE collection_tag (
            collection_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            PRIMARY KEY (collection_id, tag_id),
            FOREIGN KEY (collection_id) REFERENCES collections(collection_id),
            FOREIGN KEY (tag_id) REFERENCES tags(tag_id)
        );
        CREATE VIRTUAL TABLE titles USING fts5(title);
        CREATE UNIQUE INDEX hash_index ON items (hash);
        CREATE UNIQUE INDEX tag_index ON tags (name);
        ";
        sqlx::query(init_query).execute(&mut connection).await?;

        Ok(connection)
    }

    /// Validates the strcture of a vorg db.
    ///
    /// If valid, returns no error.
    /// If not valid, returns a `InvalidDatabase` error with a message describing why.
    async fn validate_db(connection: &mut SqliteConnection) -> Result<()> {
        let table_query = "
        SELECT tbl_name from sqlite_master
        WHERE type='table' ORDER BY tbl_name;
        ";

        static EXPECTED_TABLE_NAMES: [&str; 11] = [
            "collection_item",
            "collection_tag",
            "collections",
            "items",
            "tags",
            "titles",
            "titles_config",
            "titles_content",
            "titles_data",
            "titles_docsize",
            "titles_idx",
        ];
        static VERIFY_COLUMNS: [bool; 11] = [
            true, true, true, true, true, false, false, false, false, false, false,
        ];
        static EXPECTED_COLUMNS: [(usize, [(&str, &str); 3]); 5] = [
            // collection_item
            (
                2,
                [
                    ("collection_id", "INTEGER"),
                    ("item_id", "INTEGER"),
                    ("", ""),
                ],
            ),
            // collection_tag
            (
                2,
                [
                    ("collection_id", "INTEGER"),
                    ("tag_id", "INTEGER"),
                    ("", ""),
                ],
            ),
            // collections
            (
                2,
                [
                    ("collection_id", "INTEGER"),
                    ("title_id", "INTEGER"),
                    ("", ""),
                ],
            ),
            // items
            (
                3,
                [
                    ("ext", "TEXT"),
                    ("hash", "VARCHAR(64)"),
                    ("item_id", "INTEGER"),
                ],
            ),
            // tags
            (2, [("name", "TEXT"), ("tag_id", "INTEGER"), ("", "")]),
        ];

        let result: Vec<String> = sqlx::query(table_query)
            .try_map(|row: SqliteRow| row.try_get("tbl_name"))
            .fetch_all(&mut *connection)
            .await?;
        let table_names: Vec<&str> = result.iter().map(String::as_str).collect();

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
                    msg: format!("Table {table_name} is missing from the database.",),
                    kind: ErrorKind::DB,
                });
            }
            ListCompareResult::Unexpected(table_name) => {
                return Err(Error {
                    msg: format!("Unexpected table {table_name} exists in the database."),
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
        let query = "SELECT name,type FROM pragma_table_info(?) ORDER BY name";
        let columns: Vec<(String, String)> = sqlx::query(query)
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
                    msg: format!("Column {} is missing from table {table_name}.", column.0),
                    kind: ErrorKind::DB,
                });
            }
            ListCompareResult::Unexpected(column) => {
                return Err(Error {
                    msg: format!("Unexpected column {} in table {table_name}.", column.0),
                    kind: ErrorKind::DB,
                });
            }
            ListCompareResult::Unequal(column) => {
                return Err(Error {
                    msg: format!(
                        "Column {} in table {table_name} should have type {}.",
                        column.0, column.1
                    ),
                    kind: ErrorKind::DB,
                });
            }
            ListCompareResult::Identical => (),
        }

        Ok(())
    }

    // /**
    //  * Get studio name by id
    //  */
    // async fn get_studio(&mut self, studio_id: i64) -> Result<String> {
    //     if let Some(studio) = self.studio_cache.get(&studio_id) {
    //         Ok(studio.to_owned())
    //     } else {
    //         let query = "SELECT name FROM studios WHERE studio_id=?";
    //         let studio: String = sqlx::query(query)
    //             .bind(studio_id)
    //             .try_map(|row: SqliteRow| Ok(row.try_get("name")?))
    //             .fetch_one(&mut self.connection)
    //             .await?;
    //         self.studio_cache.insert(studio_id, studio.clone());
    //         Ok(studio)
    //     }
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use test_context::{test_context, AsyncTestContext};
    use tokio::{
        test,
        time::{sleep, Duration},
    };

    struct TempFolder {
        pub path: std::path::PathBuf,
    }

    #[async_trait::async_trait]
    impl AsyncTestContext for TempFolder {
        async fn setup() -> TempFolder {
            let temp_dir = std::path::PathBuf::from("./temp");
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
    #[serial]
    #[test]
    async fn create_db_success(ctx: &TempFolder) -> Result<()> {
        // GIVEN
        let db_path = ctx.path.join("vorg.db");

        // WHEN
        DB::new(&db_path).await?;

        // THEN
        // Verify a connection can be opened
        let mut db = SqliteConnection::connect(&db_path.to_string_lossy()).await?;

        // Verify required tables
        let test_query = "
        SELECT tbl_name FROM sqlite_master
        WHERE type='table'
        AND tbl_name IN (
            'tags', 'items', 'collections', 'collection_tag', 'collection_item', 'titles'
        );
        ";
        let num_rows = sqlx::query(test_query).fetch_all(&mut db).await?.len();
        assert_eq!(num_rows, 6);

        // Verify required indices
        let test_query = "
        SELECT tbl_name FROM sqlite_master
        WHERE type='index'
        AND name IN (
            'hash_index', 'tag_index'
        );
        ";
        let num_rows = sqlx::query(test_query).fetch_all(&mut db).await?.len();
        assert_eq!(num_rows, 2);

        Ok(())
    }

    #[test_context(TempFolder)]
    #[serial]
    #[test]
    async fn create_db_failed_db(ctx: &TempFolder) -> Result<()> {
        // GIVEN
        let db_path = ctx.path.join("vorg.db");

        // Create a folder at the target db
        fs::create_dir_all(&db_path)?;

        // WHEN
        let result = DB::new(db_path).await;

        // THEN
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error.kind, ErrorKind::DB));

        Ok(())
    }

    #[test_context(TempFolder)]
    #[serial]
    #[test]
    async fn create_db_failed_io(ctx: &TempFolder) -> Result<()> {
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
        let error = result.unwrap_err();
        assert!(matches!(error.kind, ErrorKind::IO));

        Ok(())
    }

    #[test]
    async fn test_open_db() -> Result<()> {
        DB::new("resources/db/valid.db").await?;

        Ok(())
    }

    // #[test]
    // async fn test_open_db_invalid1() {
    //     let result = DB::new("resources/db/invalid-too-many-table.db").await;
    //     assert!(result.is_err());
    //     assert_eq!(
    //         result.unwrap_err().to_string(),
    //         "DB has more tables than expected."
    //     );
    // }

    // #[test]
    // async fn test_open_db_invalid2() {
    //     let result = DB::new("resources/db/invalid-unexpected-table.db").await;
    //     assert!(result.is_err());
    //     assert_eq!(
    //         result.unwrap_err().to_string(),
    //         "Unexpected table: invalid_table."
    //     );
    // }

    // #[test]
    // async fn test_open_db_invalid3() {
    //     let result = DB::new("resources/db/invalid-not-enough-table.db").await;
    //     assert!(result.is_err());
    //     assert_eq!(
    //         result.unwrap_err().to_string(),
    //         "DB has less tables than expected."
    //     );
    // }

    // #[test]
    // async fn test_open_db_invalid4() {
    //     let result = DB::new("resources/db/invalid-too-many-column.db").await;
    //     assert!(result.is_err());
    //     assert_eq!(
    //         result.unwrap_err().to_string(),
    //         "Table items has more columns than expected."
    //     );
    // }

    // #[test]
    // async fn test_open_db_invalid5() {
    //     let result = DB::new("resources/db/invalid-unexpected-column.db").await;
    //     assert!(result.is_err());
    //     assert_eq!(
    //         result.unwrap_err().to_string(),
    //         "Table items has unexpected column: invalid_column, VARCHAR(64)"
    //     );
    // }

    // #[test]
    // async fn test_open_db_invalid6() {
    //     let result = DB::new("resources/db/invalid-not-enough-column.db").await;
    //     assert!(result.is_err());
    //     assert_eq!(
    //         result.unwrap_err().to_string(),
    //         "Table items has less columns than expected."
    //     );
    // }

    // #[test]
    // async fn test_db_import_file() -> Result<()> {
    //     let _f = TestFixture::new("temp/db_import_file");

    //     let db_path = "temp/db_import_file/vorg.db";
    //     let mut db = DB::new(db_path).await.unwrap();

    //     // Import file
    //     let title = String::from("Test title");
    //     let ext = String::from("mp4");
    //     let hash = String::from("09c683231bb0e88e84a8408fdbfe174c70d83d03e0604eb612631e79");
    //     let result = db.import_file(&title, &ext, &hash).await;
    //     assert!(result.is_ok());

    //     // Test file has been imported
    //     let mut connection = SqliteConnection::connect(db_path).await?;
    //     let test_query = "
    //         SELECT hash FROM items i, item_tag it, tags t
    //         WHERE i.item_id=it.item_id
    //         AND it.tag_id=t.tag_id
    //         AND t.name='Incomplete'
    //         AND title='Test title'
    //         AND ext='mp4'
    //         AND hash='09c683231bb0e88e84a8408fdbfe174c70d83d03e0604eb612631e79'
    //     ";
    //     assert_eq!(
    //         sqlx::query(test_query)
    //             .fetch_all(&mut connection)
    //             .await?
    //             .len(),
    //         1
    //     );

    //     // Test duplicate import
    //     let result = db.import_file(&title, &ext, &hash).await;
    //     assert!(result.is_err());
    //     assert_eq!(
    //         result.unwrap_err().to_string(),
    //         "The item to import already exists in the database."
    //     );

    //     // Test reusing tag
    //     let hash2 = String::from("4effadeed3957d9dab1a645b9a7d01c18380d54e71d51148fdf84633");
    //     let title2 = String::from("Some title");
    //     let result = db.import_file(&title2, &ext, &hash2).await;
    //     assert!(result.is_ok());
    //     let test_query = "
    //     SELECT hash FROM items i, item_tag it, tags t
    //     WHERE i.item_id=it.item_id
    //     AND it.tag_id=t.tag_id
    //     AND t.name='Incomplete'
    //     AND title='Some title'
    //     AND ext='mp4'
    //     AND hash='4effadeed3957d9dab1a645b9a7d01c18380d54e71d51148fdf84633'
    //     ";
    //     assert_eq!(
    //         sqlx::query(test_query)
    //             .fetch_all(&mut connection)
    //             .await?
    //             .len(),
    //         1
    //     );

    //     Ok(())
    // }

    // #[test]
    // async fn test_debug_fmt() {
    //     let _f = TestFixture::new("temp/db_debug_fmt");

    //     let db_path = "temp/db_debug_fmt/vorg.db";
    //     let db = DB::new(db_path).await.unwrap();
    //     let debug_fmt = format!("{db:?}");
    //     assert_eq!(
    //         debug_fmt,
    //         "Placeholder debug implementation for vorgrs::db::DB"
    //     );
    // }
}
