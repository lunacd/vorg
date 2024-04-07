#include <vorg_server.h>

#include <algorithm>
#include <memory>

#include <boost/log/trivial.hpp>
#include <crow.h>

#include <vorg_db.h>

constexpr int s_vorgServerPort = 8080;

namespace Vorg {
void runServer(const std::filesystem::path &repository) {
    std::unique_ptr<Db> db;
    try {
        db = std::make_unique<Db>(Db::connect(repository / "vorg.db"));
    } catch (const std::runtime_error &ex) {
        BOOST_LOG_TRIVIAL(fatal) << ex.what();
        return;
    }
    assert(db);

    crow::SimpleApp app;

    CROW_ROUTE(app, "/collections")
    ([&db]() {
        std::vector<crow::json::wvalue> collectionsJson;
        auto collections = db->getCollections();
        std::transform(
            collections.begin(), collections.end(),
            std::back_inserter(collectionsJson),
            [](const Collection &collection) { return collection.toJson(); });
        return crow::json::wvalue({{"collections", collectionsJson}});
    });

    app.port(s_vorgServerPort).multithreaded().run();
}
} // namespace Vorg
