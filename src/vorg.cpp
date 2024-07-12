#include <filesystem>
#include <iostream>

#include <boost/program_options.hpp>

#include <vorg_server.hpp>

namespace po = boost::program_options;

auto main(int argc, char *argv[]) -> int {
    // Parse command line arguments
    po::options_description globalDesc{};
    // clang-format off
    globalDesc.add_options()
        ("help", "command line help")
        ("command", po::value<std::string>(), "the command to run")
        ("sub_args", po::value<std::vector<std::string>>(), "command specific arguments");
    // clang-format on

    po::positional_options_description positionalDesc;
    positionalDesc.add("command", 1).add("sub_args", -1);

    po::variables_map globalOptions;
    po::store(po::command_line_parser(argc, argv)
                  .options(globalDesc)
                  .positional(positionalDesc)
                  .run(),
              globalOptions);
    po::notify(globalOptions);

    // Help
    if (globalOptions.contains("help") || !globalOptions.contains("command")) {
        std::cout << R"(Vorg file manager:
  vorg [options] [command]
Commands:
  server		run vorg web interface
)";
        std::cout << globalDesc;
        return 0;
    }

    std::vector<std::string> subArgs;
    if (globalOptions.contains("sub_args")) {
        subArgs = globalOptions.at("sub_args").as<std::vector<std::string>>();
    }

    // Server
    if (globalOptions.at("command").as<std::string>() == "server") {
        po::options_description serverDesc{};
        // clang-format off
        serverDesc.add_options()
            ("repository", po::value<std::string>(), "path to a vorg repository");
        // clang-format on
        po::positional_options_description positionalServerDesc;
        positionalServerDesc.add("repository", 1);

        po::variables_map serverOptions;
        po::store(po::command_line_parser(subArgs)
                      .options(serverDesc)
                      .positional(positionalServerDesc)
                      .run(),
                  serverOptions);
        po::notify(serverOptions);

        if (!serverOptions.contains("repository")) {
            std::cout << R"(Run vorg server:
  vorg server [repository]
)";
            return 0;
        }
        std::filesystem::path repoPath{
            serverOptions.at("repository").as<std::string>()};

        Vorg::Server server{};
        server.run();
    }

    return 0;
}
