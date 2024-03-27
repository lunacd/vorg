#include <vorg_server.h>

#include <crow.h>

constexpr int s_vorgServerPort = 8080;

namespace Vorg {
void runServer() {
    crow::SimpleApp app;

    CROW_ROUTE(app, "/")([]() {
        
        
        return "Hello world"; });

    app.port(s_vorgServerPort).multithreaded().run();
}
} // namespace Vorg
