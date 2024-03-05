#ifndef VORG_DB_H
#define VORG_DB_H

#include <SQLiteCpp/SQLiteCpp.h>

#include <filesystem>

namespace Vorg {
class Db {
  public:
    Db() = delete;
    static auto connect(const std::filesystem::path &dbPath) -> Db;

  private:
    explicit Db(SQLite::Database &&connection);

    SQLite::Database m_connection;
};
} // namespace Vorg

#endif