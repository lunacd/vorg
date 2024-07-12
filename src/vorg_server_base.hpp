#ifndef VORG_SERVER_BASE_HPP
#define VORG_SERVER_BASE_HPP

#include <boost/asio/awaitable.hpp>
#include <boost/asio/co_spawn.hpp>
#include <boost/asio/ip/tcp.hpp>
#include <boost/beast/core.hpp>
#include <boost/beast/http.hpp>
#include <boost/json.hpp>
#include <functional>
#include <unordered_map>
#include <variant>

namespace Vorg {
namespace Responses {
struct NotFound {
    std::string message;
};
struct ServerError {
    std::string message;
};
struct InvalidRequest {
    std::string message;
};
struct Json {
    boost::json::object payload;
};
} // namespace Responses

using Response = std::variant<Responses::ServerError, Responses::NotFound,
                              Responses::InvalidRequest, Responses::Json>;

class ServerBase {
  public:
    using Request =
        boost::beast::http::request<boost::beast::http::string_body>;
    using Handler = std::function<Response(Request &&req)>;
    using TcpStream = boost::beast::tcp_stream::rebind_executor<
        boost::asio::use_awaitable_t<>::executor_with_default<
            boost::asio::any_io_executor>>::other;

    void run();
    void registerHandler(boost::beast::http::verb method,
                         const std::string &route, Handler handler);

  private:
    static constexpr int s_sessionExpirationSeconds = 30;
    std::unordered_map<std::string, Handler> m_handlers;

    auto handleRequest(
        boost::beast::http::request<boost::beast::http::string_body> &&req)
        -> boost::beast::http::message_generator;
    auto doSession(TcpStream stream) -> boost::asio::awaitable<void>;
    auto doListen(boost::asio::ip::tcp::endpoint endpoint)
        -> boost::asio::awaitable<void>;
    static auto handleUnknownRoute(const Request &req) -> Response;
    static auto getHandlerKey(boost::beast::http::verb method,
                              std::string_view route) -> std::string;
};
} // namespace Vorg

#endif
