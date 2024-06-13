import argparse
import logging

# Set up logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

# Create the parser
parser = argparse.ArgumentParser(description="Run tests on server and client")

# Add the arguments
parser.add_argument("server_hostname", type=str, help="The hostname of the server")
parser.add_argument("server_interfacename", type=str, help="The interface name of the server")
parser.add_argument("client_hostname", type=str, help="The hostname of the client")
parser.add_argument("client_interfacename", type=str, help="The interface name of the client")

# Add optional arguments
parser.add_argument("-t", "--tests", type=str, nargs='*', help="List of tests to run")

# Parse the arguments
args = parser.parse_args()

# Use the arguments
logging.info(f"Server hostname: {args.server_hostname}")
logging.info(f"Server interface name: {args.server_interfacename}")
logging.info(f"Client hostname: {args.client_hostname}")
logging.info(f"Client interface name: {args.client_interfacename}")
if args.tests:
    logging.info(f"Tests to run: {args.tests}")

# All scripts, besides the nperf.py script, need to be executed on the measurement host
# The nperf script coordinates the measurements directly from the host
