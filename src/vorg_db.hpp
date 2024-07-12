#ifndef VORG_DB_HPP
#define VORG_DB_HPP

#include <filesystem>
#include <vector>

#include <SQLiteCpp/SQLiteCpp.h>

#include <models/vorg_collection.hpp>

namespace Vorg {
class Db {
  public:
    Db() = delete;
    static auto connect(const std::filesystem::path &dbPath) -> Db;

    auto getCollections() -> std::vector<Collection>;

  private:
    explicit Db(SQLite::Database &&connection);

    SQLite::Database m_connection;
};
} // namespace Vorg

#endif
