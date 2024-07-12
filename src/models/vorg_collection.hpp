#ifndef VORG_MODELS_COLLECTION_H
#define VORG_MODELS_COLLECTION_H

#include <string>
#include <vector>

// #include <crow.h>

#include <models/vorg_item.hpp>

namespace Vorg {
class Collection {
  public:
    Collection(int id, std::string title, std::vector<Item> items)
        : m_id{id}, m_title{std::move(title)}, m_items{std::move(items)} {};

    [[nodiscard]] auto title() const -> const std::string & { return m_title; };
    [[nodiscard]] auto items() const -> const std::vector<Item> & {
        return m_items;
    }

    [[nodiscard]] auto operator==(const Collection &rhs) const -> bool {
        return m_id == rhs.m_id && m_title == rhs.m_title &&
               m_items == rhs.m_items;
    }

   // [[nodiscard]] auto toJson() const -> crow::json::wvalue;

  private:
    int m_id;
    std::string m_title;
    std::vector<Item> m_items;
};
} // namespace Vorg

#endif
