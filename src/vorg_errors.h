#ifndef VORG_UTILS_ERROR_H
#define VORG_UTILS_ERROR_H

#include <string>

namespace Vorg::Utils {
class Error {
  public:
    static const std::string s_dbDoesNotExist;
};
} // namespace Vorg::Utils

#endif
