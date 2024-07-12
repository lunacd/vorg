#include <models/vorg_item.hpp>

#include <filesystem>

namespace Vorg {
auto Item::storePath() const -> std::string {
    return m_hash.substr(0, 2) + std::filesystem::path::preferred_separator +
           m_hash.substr(2) + "." + m_ext;
}

// auto Item::toJson() const -> crow::json::wvalue {
//     return crow::json::wvalue({{"path", storePath()}});
// }
} // namespace Vorg
