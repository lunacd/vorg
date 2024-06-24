#include <models/vorg_collection.h>

#include <algorithm>

namespace Vorg {
// auto Collection::toJson() const -> crow::json::wvalue {
//     std::vector<crow::json::wvalue> itemsJson;
//     std::transform(m_items.begin(), m_items.end(),
//                    std::back_inserter(itemsJson),
//                    [](const Item &item) { return item.toJson(); });
//     return crow::json::wvalue(
//         {{"id", m_id}, {"title", m_title}, {"items", itemsJson}});
// }
} // namespace Vorg
