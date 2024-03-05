#include <iostream>

#include <boost/program_options.hpp>

#include <vorg_server.h>

auto main(int argc, char *argv[]) -> int {
    // Parse command line arguments
    boost::program_options::options_description globalOptions{};
    globalOptions.add_options()("help", "command line help")(
        "command", "the command to run");

    boost::program_options::positional_options_description positionalOptions;
    positionalOptions.add("command", 1);

    boost::program_options::variables_map options;
    boost::program_options::store(
        boost::program_options::command_line_parser(argc, argv)
            .options(globalOptions)
            .positional(positionalOptions)
            .run(),
        options);
    boost::program_options::notify(options);

    // Help
    if (options.contains("help") || !options.contains("command")) {
        std::cout << "Vorg file manager:\n";
        std::cout << "  vorg [options] [command]\n";
        std::cout << globalOptions;
        std::cout << "Commands:\n";
        std::cout << "  server\t\trun vorg web interface\n";

        return 0;
    }

    // Server
    if (options.at("command").as<std::string>() == "server") {
        Vorg::runServer();
    }

    return 0;
}