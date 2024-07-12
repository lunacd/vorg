#include <vorg_server_base.hpp>

#include <algorithm>
#include <boost/asio/awaitable.hpp>
#include <boost/asio/co_spawn.hpp>
#include <boost/asio/ip/tcp.hpp>
#include <boost/asio/use_awaitable.hpp>
#include <boost/beast/core.hpp>
#include <boost/beast/http.hpp>
#include <boost/beast/version.hpp>
#include <boost/config.hpp>
#include <cstdlib>
#include <iostream>
#include <string>
#include <thread>
#include <vector>

namespace beast = boost::beast;
namespace http = beast::http;
namespace asio = boost::asio;
namespace json = boost::json;
using tcp = asio::ip::tcp;

namespace Vorg {
void ServerBase::run() {
    // Get number of processors
    auto const numThreads =
        std::max(1, static_cast<int>(std::thread::hardware_concurrency()));
    asio::io_context ioc{numThreads};

    // Listen on localhost:8000
    auto resolver = tcp::resolver(ioc);
    auto endpoints = resolver.resolve("localhost", "");
    auto const address = endpoints.begin()->endpoint().address();
    auto const port = 8000;

    boost::asio::co_spawn(
        ioc, doListen(tcp::endpoint{address, port}), [](std::exception_ptr ex) {
            if (ex) {
                try {
                    std::rethrow_exception(ex);
                } catch (std::exception &ex) {
                    std::cerr << "Error in acceptor: " << ex.what() << "\n";
                }
            }
        });

    // Run the I/O service on nproc number of threads
    std::vector<std::thread> threads;
    threads.reserve(numThreads - 1);
    for (auto i = numThreads - 1; i > 0; --i) {
        threads.emplace_back([&ioc] { ioc.run(); });
    }
    ioc.run();
}

void ServerBase::registerHandler(http::verb method, const std::string &route,
                                 Handler handler) {
    auto handlerKey = getHandlerKey(method, route);
    m_handlers.emplace(handlerKey, handler);
}

auto ServerBase::handleRequest(http::request<http::string_body> &&req)
    -> http::message_generator {
    const auto handlerKey = getHandlerKey(req.method(), req.target());
    const auto handlerIt = m_handlers.find(handlerKey);
    const Handler &handler =
        handlerIt == m_handlers.end() ? handleUnknownRoute : handlerIt->second;

    auto response = handler(std::move(req));

    struct ResponseVisitor {
      public:
        auto operator()(const Responses::NotFound &res) const
            -> http::response<http::string_body> {
            return buildResponse(http::status::not_found, ContentType::Html,
                                 res.message);
        }
        auto operator()(const Responses::ServerError &res) const
            -> http::response<http::string_body> {
            return buildResponse(http::status::internal_server_error,
                                 ContentType::Html, res.message);
        }
        auto operator()(const Responses::InvalidRequest &res) const
            -> http::response<http::string_body> {
            return buildResponse(http::status::bad_request, ContentType::Html,
                                 res.message);
        }
        auto operator()(const Responses::Json &res) const
            -> http::response<http::string_body> {
            return buildResponse(http::status::bad_request, ContentType::Json,
                                 json::serialize(res.payload));
        }

      private:
        class ContentType {
          public:
            enum ContentTypeEnum { Html, Json };
            explicit operator std::string() const {
                switch (m_value) {
                case Html:
                    return "text/html";
                case Json:
                    return "application/json";
                default:
                    return "unknown value";
                }
            }
            ContentType(const ContentTypeEnum value) : m_value(value) {}

          private:
            ContentTypeEnum m_value;
        };

        static auto buildResponse(http::status status, ContentType content_type,
                                  std::string_view body)
            -> http::response<http::string_body> {
            http::response<http::string_body> bRes{};
            bRes.result(status);
            bRes.set(http::field::content_type,
                     static_cast<std::string>(content_type));
            bRes.body() = body;
            bRes.prepare_payload();
            return bRes;
        }
    };

    // Construct beast response from handler response
    auto bRes = std::visit(ResponseVisitor{}, response);

    // Set common response properties
    bRes.set(http::field::server, BOOST_BEAST_VERSION_STRING);
    bRes.keep_alive(req.keep_alive());
    bRes.version(req.version());

    // Remove body if it is a HEAD request
    if (req.method() == http::verb::head) {
        bRes.body() = "";
    }
    return bRes;
}

// Handles an HTTP server connection
auto ServerBase::doSession(TcpStream stream) -> asio::awaitable<void> {
    // This buffer is required to persist across reads
    beast::flat_buffer buffer;

    try {
        while (true) {
            // Timeout stream after 30 seconds
            stream.expires_after(
                std::chrono::seconds(s_sessionExpirationSeconds));

            // Receive request
            http::request<http::string_body> req;
            co_await http::async_read(stream, buffer, req);

            // Handle request
            http::message_generator msg = handleRequest(std::move(req));

            bool keepAlive = msg.keep_alive();

            // Send response
            co_await beast::async_write(stream, std::move(msg),
                                        asio::use_awaitable);

            if (!keepAlive) {
                // Close connection
                break;
            }
        }
    } catch (boost::system::system_error &ex) {
        if (ex.code() != http::error::end_of_stream) {
            throw;
        }
    }

    // Shutdown TCP stream
    // Ignore errors because the stream might have already been closed
    stream.socket().shutdown(tcp::socket::shutdown_send);
}

// Accepts incoming connections and launches the sessions
auto ServerBase::doListen(tcp::endpoint endpoint) -> asio::awaitable<void> {
    // Open the acceptor
    auto acceptor = asio::use_awaitable.as_default_on(
        tcp::acceptor(co_await asio::this_coro::executor));
    acceptor.open(endpoint.protocol());

    // Allow address reuse
    acceptor.set_option(asio::socket_base::reuse_address(true));

    // Bind to the server address
    acceptor.bind(endpoint);

    // Start listening for connections
    acceptor.listen(asio::socket_base::max_listen_connections);

    // Spawn doSession on each successful connection
    while (true) {
        asio::co_spawn(acceptor.get_executor(),
                       doSession(TcpStream(co_await acceptor.async_accept())),
                       [](std::exception_ptr ex) {
                           if (ex) {
                               try {
                                   std::rethrow_exception(ex);
                               } catch (std::exception &ex) {
                                   std::cerr
                                       << "Error in session: " << ex.what()
                                       << "\n";
                               }
                           }
                       });
    }
}

auto ServerBase::handleUnknownRoute(const Request &req) -> Response {
    return Response{Responses::NotFound{
        std::format("Route {} is not found.", std::string_view(req.target()))}};
}
auto ServerBase::getHandlerKey(http::verb method, std::string_view route)
    -> std::string {
    auto verbStr = std::to_string(static_cast<int>(method));
    auto key = std::format("{} {}", verbStr, route);
    return key;
}
} // namespace Vorg
