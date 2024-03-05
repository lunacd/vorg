#include <vorg_server.h>

#include <functional>

#include <drogon/drogon.h>
#include <inja/inja.hpp>

namespace Vorg {
constexpr int s_vorgPort = 8080;
constexpr int s_numThreads = 2;

void runServer() {
    drogon::app().loadConfigFile("./runtime/drogon.json");

    // Landing page
    drogon::app().registerHandler(
        "/",
        [](const drogon::HttpRequestPtr &,
           std::function<void(const drogon::HttpResponsePtr &)> &&callback) {
            auto res = drogon::HttpResponse::newHttpResponse();
            res->setBody("hello world");
            callback(res);
        });

    drogon::app().run();
}
} // namespace Vorg
