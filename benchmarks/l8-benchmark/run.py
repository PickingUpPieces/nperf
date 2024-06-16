import argparse
import logging

TESTS = ['nperf', 'sysinfo', 'iperf2', 'iperf3', 'netperf']

# Set up logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

def main():
    logging.info('Starting main function')

    # Create the parser
    parser = argparse.ArgumentParser(description="Run tests on server and client")

    # Add the arguments
    parser.add_argument("server_hostname", type=str, help="The hostname of the server")
    parser.add_argument("server_interfacename", type=str, help="The interface name of the server")
    parser.add_argument("client_hostname", type=str, help="The hostname of the client")
    parser.add_argument("client_interfacename", type=str, help="The interface name of the client")

    # Add optional arguments
    parser.add_argument("-t", "--tests", type=str, nargs='*', help="List of tests to run in a string with space separated values. Possible values: nperf, sysinfo, iperf2, iperf3, netperf")

    # Parse the arguments
    args = parser.parse_args()

    # Use the arguments
    logging.info(f"Server hostname: {args.server_hostname}")
    logging.info(f"Server interface name: {args.server_interfacename}")
    logging.info(f"Client hostname: {args.client_hostname}")
    logging.info(f"Client interface name: {args.client_interfacename}")

    if args.tests:
        passed_tests = args.tests.split()
        tests = []
        for test in passed_tests:
            if test not in TESTS:
                logging.warning(f"Invalid test: {test}")
            else:
                logging.info(f"Running test: {test}")
                # Add the test to the tests list
                tests.append(test)
    else:
        logging.info("All tests are run")
        tests = TESTS

    execute_tests(tests)


def execute_tests(tests: list) -> bool:
    return True

if __name__ == '__main__':
    logging.info('Starting script')
    main()
    logging.info('Script finished')
