#include <vorg_db.h>

#include <array>
#include <cassert>
#include <filesystem>
#include <stdexcept>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

#include <SQLiteCpp/Database.h>
#include <SQLiteCpp/Statement.h>

#include <models/vorg_collection.h>

namespace Vorg {
namespace {
void vorgCreateDb(SQLite::Database &connection) {
    const char *createDbStmt = R"(
        CREATE TABLE tags (
            tag_id INTEGER PRIMARY KEY NOT NULL,
            name TEXT NOT NULL
        );
        CREATE TABLE collections (
            collection_id INTEGER PRIMARY KEY NOT NULL,
            title TEXT NOT NULL
        );
        CREATE TABLE items (
            collection_id INTEGER NOT NULL,
            item_id INTEGER PRIMARY KEY NOT NULL,
            hash VARCHAR(64) NOT NULL,
            ext TEXT NOT NULL,
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
        CREATE UNIQUE INDEX hash_index ON items (hash);
        CREATE UNIQUE INDEX tag_index ON tags (name);
        CREATE TRIGGER title_insert AFTER INSERT ON collections
        BEGIN
            INSERT INTO title_fts(rowid, title) VALUES (new.collection_id, new.title);
        END;
        CREATE TRIGGER title_delete AFTER DELETE ON collections
        BEGIN
            INSERT INTO title_fts(title_fts, rowid, title)
                VALUES('delete', old.collection_id, old.title);
        END;
        CREATE TRIGGER title_update AFTER UPDATE ON collections
        BEGIN
            INSERT INTO title_fts(fts_idx, rowid, title) VALUES('delete', old.collection_id, old.title);
            INSERT INTO title_fts(rowid, title) VALUES (new.collection_id, new.title);
        END;
    )";
    connection.exec(createDbStmt);
}

auto vorgValidateTableColumns(const SQLite::Database &connection,
                              const std::string &tableName) -> bool {
    static const std::unordered_map<
        std::string, std::vector<std::pair<std::string, std::string>>>
        expectedTableColumns = {{"collections",
                                 {
                                     {"collection_id", "INTEGER"},
                                     {"title", "TEXT"},
                                 }},
                                {"collection_tag",
                                 {
                                     {"collection_id", "INTEGER"},
                                     {"tag_id", "INTEGER"},
                                 }},
                                {"items",
                                 {
                                     {"collection_id", "INTEGER"},
                                     {"ext", "TEXT"},
                                     {"hash", "VARCHAR(64)"},
                                     {"item_id", "INTEGER"},
                                 }},
                                {"tags",
                                 {
                                     {"name", "TEXT"},
                                     {"tag_id", "INTEGER"},
                                 }}};

    auto tableColumnIt = expectedTableColumns.find(tableName);

    // Unexpected table name
    // Caller should validate table name before calling this function
    assert(tableColumnIt != expectedTableColumns.end());

    SQLite::Statement tableStmt{connection, R"(
        SELECT name,type FROM pragma_table_info(?) ORDER BY name
    )"};
    tableStmt.bind(1, tableName);

    for (const auto &columnPair : tableColumnIt->second) {
        if (!tableStmt.executeStep()) {
            // Missing column
            return false;
        }
        const std::string columnName = tableStmt.getColumn("name");
        const std::string columnType = tableStmt.getColumn("type");

        if (columnName != columnPair.first) {
            // Incorrect column name
            return false;
        }
        if (columnType != columnPair.second) {
            // Incorrect column type
            return false;
        }
    }

    return !tableStmt.executeStep();
}

auto vorgValidateDb(const SQLite::Database &connection) -> bool {
    // Expected table data
    constexpr std::array<const char *, 4> s_expectedTableNames = {
        "collection_tag", "collections", "items", "tags"};
    constexpr std::array<const char *, 2> s_expectedIndexNames = {"hash_index",
                                                                  "tag_index"};
    constexpr std::array<const char *, 3> s_expectedTriggerNames = {
        "title_delete", "title_insert", "title_update"};
    constexpr int s_expectedFtsTableCount = 5;

    // Check table count and names
    // Get all tables except fts tables
    SQLite::Statement tableNameStmt{connection, R"(
            SELECT tbl_name from sqlite_master
            WHERE type='table' AND tbl_name NOT LIKE 'title_fts%'
            ORDER BY tbl_name
        )"};
    for (const auto &expectedTableName : s_expectedTableNames) {
        if (!tableNameStmt.executeStep()) {
            return false;
        }
        const std::string tableName = tableNameStmt.getColumn("tbl_name");

        // Validate table names are the same
        if (tableName != expectedTableName) {
            return false;
        }

        // Check column count, type, and name
        if (!vorgValidateTableColumns(connection, tableName)) {
            return false;
        }
    }
    if (tableNameStmt.executeStep()) {
        // Unexpected extra tables at the end
        return false;
    }

    // Check fts table count
    SQLite::Statement ftsTableStmt{connection, R"(
        SELECT count(tbl_name) AS fts_count from sqlite_master
        WHERE type='table' AND tbl_name LIKE 'title_fts%'
    )"};

    // Not checking return value because count will return exactly 1 roa
    ftsTableStmt.executeStep();

    auto ftsCount = static_cast<int>(ftsTableStmt.getColumn("fts_count"));
    if (static_cast<int>(ftsTableStmt.getColumn("fts_count")) !=
        s_expectedFtsTableCount) {
        // Wrong number of fts table
        return false;
    }

    // Check indices
    SQLite::Statement indexStmt{connection, R"(
        SELECT name FROM sqlite_master
        WHERE type='index' AND name NOT LIKE 'sqlite_%'
        ORDER BY name
    )"};
    for (const auto &expectedIndexName : s_expectedIndexNames) {
        if (!indexStmt.executeStep()) {
            // Missing index
            return false;
        }
        std::string indexName = indexStmt.getColumn("name");

        if (indexName != expectedIndexName) {
            // Wrong index name
            return false;
        }
    }
    if (indexStmt.executeStep()) {
        return false;
    }

    // Check triggers
    SQLite::Statement triggerStmt{connection, R"(
        SELECT name FROM sqlite_master
        WHERE type='trigger' ORDER BY name
    )"};
    for (const auto &expectedTriggerName : s_expectedTriggerNames) {
        if (!triggerStmt.executeStep()) {
            // Missing trigger
            return false;
        }
        std::string triggerName = triggerStmt.getColumn("name");

        if (triggerName != expectedTriggerName) {
            // Wrong trigger name
            return false;
        }
    }
    return !triggerStmt.executeStep();
}
} // namespace

auto Db::connect(const std::filesystem::path &dbPath) -> Db {
    const bool dbExists = std::filesystem::exists(dbPath);

    if (dbExists) {
        // If database exists, validate it before constructing DB
        SQLite::Database connection{dbPath.string(), SQLite::OPEN_READWRITE};
        if (!vorgValidateDb(connection)) {
            throw std::runtime_error("The vorg database is corrupted.");
        }
        return Db{std::move(connection)};
    }

    // If database does not exist, construct a new one instead
    SQLite::Database connection{dbPath.string(),
                                SQLite::OPEN_READWRITE | SQLite::OPEN_CREATE};
    vorgCreateDb(connection);
    return Db{std::move(connection)};
}

auto Db::getCollections() -> std::vector<Collection> {
    SQLite::Transaction transaction{m_connection};
    SQLite::Statement getCollectionsStmt{m_connection, R"(
        SELECT collection_id, title FROM collections 
    )"};
    SQLite::Statement getItemsStmt{m_connection, R"(
        SELECT item_id, ext, hash FROM items WHERE collection_id=?
    )"};
    std::vector<Collection> collections;
    while (getCollectionsStmt.executeStep()) {
        int collectionId =
            getCollectionsStmt.getColumn("collection_id").getInt();
        std::string title = getCollectionsStmt.getColumn("title").getString();

        std::vector<Item> items;
        getItemsStmt.bind(1, collectionId);
        while (getItemsStmt.executeStep()) {
            std::string hash = getItemsStmt.getColumn("hash").getString();
            std::string ext = getItemsStmt.getColumn("ext").getString();
            items.emplace_back(hash, ext);
        }
        getItemsStmt.reset();

        collections.emplace_back(collectionId, std::move(title),
                                 std::move(items));
    }
    transaction.commit();
    return collections;
}

Db::Db(SQLite::Database &&connection) : m_connection{std::move(connection)} {}
} // namespace Vorg