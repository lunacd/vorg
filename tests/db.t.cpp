
#include <gmock/gmock.h>
#include <gtest/gtest.h>

#include <filesystem>

#include <SQLiteCpp/Database.h>
#include <SQLiteCpp/Statement.h>
#include <boost/uuid/uuid.hpp>
#include <boost/uuid/uuid_generators.hpp>
#include <boost/uuid/uuid_io.hpp>

#include <models/vorg_item.h>
#include <vorg_db.h>

using namespace ::testing;

namespace Vorg ::Tests {

class DbTestFixture : public Test {
  public:
    void SetUp() override {
        boost::uuids::uuid uuid = boost::uuids::random_generator()();
        std::filesystem::path tempPath =
            std::filesystem::current_path() / "temp";
        std::filesystem::create_directory(tempPath);
        m_dbPath = tempPath / (boost::uuids::to_string(uuid) + ".db");
        m_collections.emplace_back(
            1, "abc",
            std::vector<Item>{Item{
                                  "a0d2139fbc5efd9174211f5ade3a2e44fec969c799f1"
                                  "0c16fde95ee178b4f44e",
                                  "mp4",
                              },
                              Item{
                                  "bb4208052b8abf47524be1336a002f962f518d10755c"
                                  "832d7a18050131e70749",
                                  "avi",
                              }});
        m_collections.emplace_back(
            2, "def",
            std::vector<Item>{
                Item{"47f9c6577a35c2ce250bffb97fc5879c4306be6c3dd2833b0c19728"
                     "671ef4814",
                     "wmv"},
            });
    }

    void TearDown() override {
        if (std::filesystem::exists(m_dbPath)) {
            std::filesystem::remove(m_dbPath);
        }
    }

    void bootstrapDb() const {
        const char *sql = R"(
            BEGIN TRANSACTION;
            CREATE TABLE tags (
                tag_id  INTEGER NOT NULL,
                name    TEXT NOT NULL,
                PRIMARY KEY("tag_id")
            );
            CREATE TABLE collections (
                collection_id   INTEGER NOT NULL,
                title           TEXT NOT NULL,
                PRIMARY KEY("collection_id")
            );
            CREATE TABLE collection_tag (
                collection_id   INTEGER NOT NULL,
                tag_id          INTEGER NOT NULL,
                PRIMARY KEY("collection_id","tag_id"),
                FOREIGN KEY("tag_id") REFERENCES "tags"("tag_id"),
                FOREIGN KEY("collection_id") REFERENCES "collections"("collection_id")
            );
            CREATE TABLE items (
                item_id         INTEGER NOT NULL,
                collection_id   INTEGER NOT NULL,
                ext             TEXT NOT NULL,
                hash            VARCHAR(64) NOT NULL,
                PRIMARY KEY("item_id"),
                FOREIGN KEY("collection_id") REFERENCES "collections"("collection_id")
            );
            CREATE VIRTUAL TABLE title_fts USING fts5 (
                title,
                content='collections',
                content_rowid='collection_id'
            );
            CREATE UNIQUE INDEX hash_index ON items (
                hash
            );
            CREATE UNIQUE INDEX tag_index ON tags (
                name
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
            COMMIT;
        )";
        applySql(sql);
    }

    void setupSampleData() {
        int collectionId = 1;
        auto connection = getConnection();
        const char *addCollectionSql = R"(
            INSERT INTO collections(collection_id, title)
                VALUES (?, ?);
        )";
        SQLite::Statement addCollectionStmt{connection, addCollectionSql};
        const char *addItemSql = R"(
            INSERT INTO items(collection_id, item_id, hash, ext)
                VALUES (?, ?, ?, ?);
        )";
        SQLite::Statement addItemStmt{connection, addItemSql};
        for (int collectionId = 1, itemId = 1;
             const auto &collection : m_collections) {
            addCollectionStmt.bind(1, collectionId);
            addCollectionStmt.bind(2, collection.title());
            addCollectionStmt.exec();
            addCollectionStmt.reset();

            for (const auto &collectionItem : collection.items()) {
                addItemStmt.bind(1, collectionId);
                addItemStmt.bind(2, itemId);
                addItemStmt.bind(3, collectionItem.hash());
                addItemStmt.bind(4, collectionItem.ext());
                addItemStmt.exec();
                addItemStmt.reset();

                ++itemId;
            }

            ++collectionId;
        }
    }

    void applySql(const char *sql) const {
        SQLite::Database connection(m_dbPath.string(), SQLite::OPEN_READWRITE |
                                                           SQLite::OPEN_CREATE);
        connection.exec(sql);
    }

    [[nodiscard]] auto getConnection() const -> SQLite::Database {
        return {m_dbPath.string(),
                SQLite::OPEN_READWRITE | SQLite::OPEN_CREATE};
    }

    [[nodiscard]] auto dbPath() const -> const std::filesystem::path & {
        return m_dbPath;
    }

    [[nodiscard]] auto collections() const -> const std::vector<Collection> & {
        return m_collections;
    }

  private:
    std::filesystem::path m_dbPath;
    std::vector<Collection> m_collections;
};

// NOLINTBEGIN

TEST_F(DbTestFixture, CreateFull) {
    // This test exploits the fact that constructing a Vorg::Db on a
    // non-existent file creates a vorg db and doing so again on the existent db
    // validates it.
    // Using testing source code with source code is sloppy, but duplicating the
    // complete db validation logic in unit test is equally undesirable.
    // Thus, an additional CreateBasic test is created to sanity-check the db
    // creation process.

    // WHEN
    ASSERT_NO_THROW(Db::connect(dbPath()));
    // THEN
    ASSERT_NO_THROW(Db::connect(dbPath()));
}

TEST_F(DbTestFixture, CreateBasic) {
    // WHEN
    ASSERT_NO_THROW(Db::connect(dbPath()));

    // THEN
    // Verify table, index, and trigger count
    SQLite::Database connection(dbPath().string());
    SQLite::Statement query(connection, R"(
        SELECT type, count(type) AS count from sqlite_master
        WHERE name NOT LIKE 'sqlite_%'
        GROUP BY type
        ORDER BY type
    )");
    // Index
    ASSERT_TRUE(query.executeStep());
    ASSERT_THAT(static_cast<std::string>(query.getColumn("type")), Eq("index"));
    ASSERT_THAT(static_cast<int>(query.getColumn("count")), Eq(2));
    // Table
    ASSERT_TRUE(query.executeStep());
    ASSERT_THAT(static_cast<std::string>(query.getColumn("type")), Eq("table"));
    ASSERT_THAT(static_cast<int>(query.getColumn("count")), Eq(9));
    // Index
    ASSERT_TRUE(query.executeStep());
    ASSERT_THAT(static_cast<std::string>(query.getColumn("type")),
                Eq("trigger"));
    ASSERT_THAT(static_cast<int>(query.getColumn("count")), Eq(3));
}

TEST_F(DbTestFixture, ValidateMissingTable) {
    // GIVEN
    bootstrapDb();
    const char *missingTableSql = R"(
        DROP TABLE collection_tag;
    )";
    applySql(missingTableSql);

    // WHEN
    // THEN
    EXPECT_THROW(Db::connect(dbPath()), std::runtime_error);
}

TEST_F(DbTestFixture, ValidateExtraTable) {
    // GIVEN
    bootstrapDb();
    const char *extraTableSql = R"(
        CREATE TABLE zzz (
            id  INTEGER NOT NULL
        );
    )";
    applySql(extraTableSql);

    // WHEN
    // THEN
    EXPECT_THROW(Db::connect(dbPath()), std::runtime_error);
}

TEST_F(DbTestFixture, ValidateMissingColumn) {
    // GIVEN
    bootstrapDb();
    const char *missingColumnSql = R"(
        ALTER TABLE items
        DROP COLUMN ext; 
    )";
    applySql(missingColumnSql);

    // WHEN
    // THEN
    EXPECT_THROW(Db::connect(dbPath()), std::runtime_error);
}

TEST_F(DbTestFixture, ValidateWrongColumnType) {
    // GIVEN
    bootstrapDb();
    //  Drop and add column because SQLite doesn't support ALTER COLUMN
    const char *wrongColumnTypeSql = R"(
        BEGIN TRANSACTION;
        DROP INDEX hash_index;
        ALTER TABLE items
        DROP COLUMN hash;
        ALTER TABLE items
        ADD COLUMN hash TEXT;
        CREATE UNIQUE INDEX hash_index ON items (
            hash
        );
        COMMIT;
    )";
    applySql(wrongColumnTypeSql);

    // WHEN
    // THEN
    EXPECT_THROW(Db::connect(dbPath()), std::runtime_error);
}

TEST_F(DbTestFixture, ValidateMissingFts) {
    // GIVEN
    bootstrapDb();
    const char *missingFtsSql = R"(
        DROP TABLE title_fts
    )";
    applySql(missingFtsSql);

    // WHEN
    // THEN
    EXPECT_THROW(Db::connect(dbPath()), std::runtime_error);
}

TEST_F(DbTestFixture, ValidateMissingIndex) {
    // GIVEN
    bootstrapDb();
    const char *missingIndexSql = R"(
        DROP INDEX hash_index
    )";
    applySql(missingIndexSql);

    // WHEN
    // THEN
    EXPECT_THROW(Db::connect(dbPath()), std::runtime_error);
}

TEST_F(DbTestFixture, ValidateExtraIndex) {
    // GIVEN
    bootstrapDb();
    const char *extraIndexSql = R"(
        CREATE INDEX z_index ON items (
            ext
        )
    )";
    applySql(extraIndexSql);

    // WHEN
    // THEN
    EXPECT_THROW(Db::connect(dbPath()), std::runtime_error);
}

TEST_F(DbTestFixture, ValidateMissingTrigger) {
    // GIVEN
    bootstrapDb();
    const char *missingTriggerSql = R"(
        DROP TRIGGER title_insert
    )";
    applySql(missingTriggerSql);

    // WHEN
    // THEN
    EXPECT_THROW(Db::connect(dbPath()), std::runtime_error);
}

TEST_F(DbTestFixture, ValidateExtraTrigger) {
    // GIVEN
    bootstrapDb();
    const char *extraTriggerSql = R"(
        CREATE TRIGGER z_trigger AFTER INSERT ON collections BEGIN
            INSERT INTO title_fts(rowid, title) VALUES (new.collection_id,
            new.title);
        END;
    )";
    applySql(extraTriggerSql);

    // WHEN
    // THEN
    EXPECT_THROW(Db::connect(dbPath()), std::runtime_error);
}

TEST_F(DbTestFixture, GetCollectionsBasic) {
    // GIVEN
    bootstrapDb();
    setupSampleData();

    // WHEN
    auto sut = Db::connect(dbPath());
    auto resultCollections = sut.getCollections();

    // THEN
    ASSERT_THAT(resultCollections.size(), Eq(2));
    for (size_t index = 0; const auto &collection : resultCollections) {
        ASSERT_THAT(collection, Eq(collections().at(index)));

        ++index;
    }
}
} // namespace Vorg::Tests

// NOLINTEND
