#include <vorg_server.hpp>

namespace http = boost::beast::http;
namespace json = boost::json;

namespace Vorg {
Server::Server() { registerHandler(http::verb::get, "/", helloWorld); }

auto Server::helloWorld([[maybe_unused]] Request &&req) -> Response {
    json::object payload;
    payload["abc"] = "def";
    return Responses::Json{payload};
}
} // namespace Vorg
