#ifndef VORG_SERVER_HPP
#define VORG_SERVER_HPP

#include <vorg_server_base.hpp>

namespace Vorg {
class Server : public ServerBase {
  public:
    Server();

  private:
    static auto helloWorld(Request &&req) -> Response;
};
} // namespace Vorg

#endif
