#ifndef VORG_MODELS_ITEM_H
#define VORG_MODELS_ITEM_H

#include <string>

#include <crow.h>

namespace Vorg {
class Item {
  public:
    Item(std::string hash, std::string ext)
        : m_hash{std::move(hash)}, m_ext{std::move(ext)} {}

    [[nodiscard]] auto hash() const -> const std::string & { return m_hash; }
    [[nodiscard]] auto ext() const -> const std::string & { return m_ext; }

    [[nodiscard]] auto operator==(const Item &rhs) const -> bool {
        return m_hash == rhs.m_hash && m_ext == m_ext;
    }

    [[nodiscard]] auto storePath() const -> std::string;
    [[nodiscard]] auto toJson() const -> crow::json::wvalue;

  private:
    std::string m_hash;
    std::string m_ext;
};

} // namespace Vorg

#endif
