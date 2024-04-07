#ifndef VORG_SERVER_H
#define VORG_SERVER_H

#include <filesystem>

namespace Vorg {
void runServer(const std::filesystem::path &repository);
}

#endif
